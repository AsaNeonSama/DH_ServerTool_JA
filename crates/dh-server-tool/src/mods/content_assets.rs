use std::borrow::Cow;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::consts::mods::{
    CONTENT_MOD_ARCHIVE_MAGIC, CONTENT_MOD_CACHE_SUBDIR, CONTENT_MOD_READY_MARKER,
    MAX_CONTENT_ARCHIVE_FILES,
};
use crate::structs::mods::ContentModAssetBundle;

static PROBE_SEQ: AtomicU64 = AtomicU64::new(1);

fn sha256_bytes_hex(bytes: &[u8]) -> String {
    let mut hasher: Sha256 = Sha256::new();
    hasher.update(bytes);
    let digest: [u8; 32] = hasher.finalize().into();
    let mut out: String = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn to_normal_absolute_path(path: &Path) -> Result<PathBuf, String> {
    let abs: PathBuf = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e: std::io::Error| format!("failed to query cwd: {e}"))?
            .join(path)
    };
    let s: Cow<'_, str> = abs.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        Ok(PathBuf::from(stripped))
    } else {
        Ok(abs)
    }
}

fn ensure_writable_dir(dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e: std::io::Error| {
        format!("failed to create cache dir {}: {e}", dir.display())
    })?;
    let abs: PathBuf = to_normal_absolute_path(dir)?;
    let probe_seq: u64 = PROBE_SEQ.fetch_add(1, Ordering::Relaxed);
    let probe: PathBuf = abs.join(format!(
        ".cm_write_probe_{}_{probe_seq}.tmp",
        std::process::id()
    ));
    let _file: File = File::create(&probe)
        .map_err(|e: std::io::Error| format!("cache dir not writable {}: {e}", abs.display()))?;
    let _ = fs::remove_file(&probe);
    Ok(abs)
}

fn resolve_cache_root(override_root: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(custom) = override_root {
        return ensure_writable_dir(custom);
    }
    for candidate in dh_gadget_injector::default_cache_candidates() {
        if let Ok(abs) = ensure_writable_dir(&candidate) {
            return Ok(abs);
        }
    }
    Err("no writable cache directory available for content MOD assets".to_string())
}

fn validate_safe_rel_path(rel: &str) -> Result<PathBuf, String> {
    if rel.is_empty()
        || rel.starts_with('/')
        || rel.contains('\\')
        || rel.contains(':')
        || rel.contains('\0')
    {
        return Err(format!("unsafe content MOD asset path: {rel}"));
    }
    let mut out: PathBuf = PathBuf::new();
    for segment in rel.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(format!("invalid path segment in content MOD asset: {rel}"));
        }
        out.push(segment);
    }
    Ok(out)
}

fn read_u32_le(buf: &[u8], pos: &mut usize) -> Result<u32, String> {
    let end: usize = pos
        .checked_add(4)
        .ok_or_else(|| "archive offset overflow".to_string())?;
    let slice: &[u8] = buf
        .get(*pos..end)
        .ok_or_else(|| "unexpected EOF reading u32 from content MOD archive".to_string())?;
    let arr: [u8; 4] = slice
        .try_into()
        .map_err(|_| "failed to parse u32 from content MOD archive".to_string())?;
    *pos = end;
    Ok(u32::from_le_bytes(arr))
}

fn read_u64_le(buf: &[u8], pos: &mut usize) -> Result<u64, String> {
    let end: usize = pos
        .checked_add(8)
        .ok_or_else(|| "archive offset overflow".to_string())?;
    let slice: &[u8] = buf
        .get(*pos..end)
        .ok_or_else(|| "unexpected EOF reading u64 from content MOD archive".to_string())?;
    let arr: [u8; 8] = slice
        .try_into()
        .map_err(|_| "failed to parse u64 from content MOD archive".to_string())?;
    *pos = end;
    Ok(u64::from_le_bytes(arr))
}

fn unpack_archive_into(archive: &[u8], dest_dir: &Path) -> Result<u32, String> {
    if archive.len() < 8 || &archive[0..4] != CONTENT_MOD_ARCHIVE_MAGIC {
        return Err("invalid content MOD archive magic".to_string());
    }
    let mut pos: usize = 4;
    let file_count: u32 = read_u32_le(archive, &mut pos)?;
    if file_count > MAX_CONTENT_ARCHIVE_FILES {
        return Err(format!(
            "content MOD archive file count ({file_count}) exceeds limit ({MAX_CONTENT_ARCHIVE_FILES})"
        ));
    }

    for _ in 0..file_count {
        let path_len_u32: u32 = read_u32_le(archive, &mut pos)?;
        let path_len: usize =
            usize::try_from(path_len_u32).map_err(|_| "path length exceeds usize".to_string())?;
        let path_end: usize = pos
            .checked_add(path_len)
            .ok_or_else(|| "path offset overflow".to_string())?;
        let path_bytes: &[u8] = archive
            .get(pos..path_end)
            .ok_or_else(|| "unexpected EOF reading asset path".to_string())?;
        let rel_str: &str = std::str::from_utf8(path_bytes)
            .map_err(|e: std::str::Utf8Error| format!("invalid UTF-8 asset path: {e}"))?;
        pos = path_end;

        let data_len_u64: u64 = read_u64_le(archive, &mut pos)?;
        let data_len: usize =
            usize::try_from(data_len_u64).map_err(|_| "asset length exceeds usize".to_string())?;
        let data_end: usize = pos
            .checked_add(data_len)
            .ok_or_else(|| "data offset overflow".to_string())?;
        let data: &[u8] = archive
            .get(pos..data_end)
            .ok_or_else(|| "unexpected EOF reading asset bytes".to_string())?;
        pos = data_end;

        let rel_path: PathBuf = validate_safe_rel_path(rel_str)?;
        let full_path: PathBuf = dest_dir.join(rel_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).map_err(|e: std::io::Error| {
                format!("failed to create dir {}: {e}", parent.display())
            })?;
        }
        fs::write(&full_path, data).map_err(|e: std::io::Error| {
            format!("failed to write asset {}: {e}", full_path.display())
        })?;
    }

    if pos != archive.len() {
        return Err(format!(
            "trailing bytes in content MOD archive ({pos} != {})",
            archive.len()
        ));
    }

    Ok(file_count)
}

/// Extracts an embedded server-side Content MOD asset bundle into the local cache
/// (`%LOCALAPPDATA%\DH_ServerTool_JA\content_mods\<mod_id>_<sha12>`), verifying SHA-256
/// and reusing existing verified extractions across launches.
pub fn extract_content_mod_assets(
    bundle: &ContentModAssetBundle,
    override_cache_root: Option<&Path>,
) -> Result<PathBuf, String> {
    let expected_sha: &str = bundle.archive_sha256.trim();
    let short_hash: &str = expected_sha.get(..12).unwrap_or(expected_sha);

    let cache_root: PathBuf = resolve_cache_root(override_cache_root)?;
    let content_root: PathBuf = cache_root.join(CONTENT_MOD_CACHE_SUBDIR);
    fs::create_dir_all(&content_root).map_err(|e: std::io::Error| {
        format!(
            "failed to create content mods cache root {}: {e}",
            content_root.display()
        )
    })?;

    let target_dir: PathBuf = content_root.join(format!("{}_{short_hash}", bundle.mod_id));
    let marker_path: PathBuf = target_dir.join(CONTENT_MOD_READY_MARKER);
    if target_dir.is_dir()
        && fs::read_to_string(&marker_path)
            .is_ok_and(|content: String| content.trim() == expected_sha)
    {
        return Ok(target_dir);
    }

    let archive: Vec<u8> = zstd::stream::decode_all(bundle.archive_zst)
        .map_err(|e: std::io::Error| format!("failed to decompress {}: {e}", bundle.mod_id))?;
    let actual_sha: String = sha256_bytes_hex(&archive);
    if actual_sha != expected_sha {
        return Err(format!(
            "content MOD archive SHA-256 mismatch for {}: expected {expected_sha}, got {actual_sha}",
            bundle.mod_id
        ));
    }

    let mut suffix: [u8; 4] = [0u8; 4];
    rand::rngs::OsRng.fill_bytes(&mut suffix);
    let tmp_dir: PathBuf = content_root.join(format!(
        ".{}_{short_hash}.{}.{:02x}{:02x}{:02x}{:02x}.tmp",
        bundle.mod_id,
        std::process::id(),
        suffix[0],
        suffix[1],
        suffix[2],
        suffix[3],
    ));
    if tmp_dir.exists() {
        let _ = fs::remove_dir_all(&tmp_dir);
    }
    fs::create_dir_all(&tmp_dir).map_err(|e: std::io::Error| {
        format!("failed to create temp dir {}: {e}", tmp_dir.display())
    })?;

    if let Err(err) = unpack_archive_into(&archive, &tmp_dir) {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(err);
    }
    if let Err(err) = fs::write(tmp_dir.join(CONTENT_MOD_READY_MARKER), expected_sha) {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(format!("failed to write ready marker: {err}"));
    }

    if target_dir.exists() {
        let _ = fs::remove_dir_all(&target_dir);
    }
    if let Err(rename_err) = fs::rename(&tmp_dir, &target_dir) {
        let now_ready: bool = target_dir.is_dir()
            && fs::read_to_string(&marker_path)
                .is_ok_and(|content: String| content.trim() == expected_sha);
        let _ = fs::remove_dir_all(&tmp_dir);
        if !now_ready {
            return Err(format!(
                "failed to finalize extracted assets at {}: {rename_err}",
                target_dir.display()
            ));
        }
    }

    Ok(target_dir)
}

/// Converts a filesystem path into a safe single-quoted JS string value with forward slashes.
#[must_use]
pub fn format_js_single_quoted_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "\\'")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}
