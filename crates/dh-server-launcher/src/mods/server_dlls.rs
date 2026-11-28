use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::consts::server_dlls::{
    EMBEDDED_VOICE_SERVER_DLL_ZST, SERVER_DLL_CACHE_APP_DIR, SERVER_DLL_CACHE_FALLBACK_DIR,
    SERVER_DLL_CACHE_SUBDIR, SHA256_READ_BUFFER_SIZE, VOICE_SERVER_DLL_FILE_NAME,
    VOICE_SERVER_DLL_RAW_LEN, VOICE_SERVER_DLL_SHA256, VOICE_SERVER_DLL_SUBDIR_PREFIX,
};
use crate::languages;
use crate::mods::registry::find_content_option_by_number;
use crate::structs::languages::ModLabelCode;
use crate::structs::mods::{ExtractedServerDll, ModMenuOption, ServerBuiltinDllId};

static WRITE_SEQ: AtomicU64 = AtomicU64::new(1);

impl ServerBuiltinDllId {
    #[must_use]
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::VoiceServer => VOICE_SERVER_DLL_FILE_NAME,
        }
    }

    #[must_use]
    pub const fn label_code(self) -> ModLabelCode {
        match self {
            Self::VoiceServer => ModLabelCode::ContentUeVoice,
        }
    }

    #[must_use]
    pub const fn subdir_prefix(self) -> &'static str {
        match self {
            Self::VoiceServer => VOICE_SERVER_DLL_SUBDIR_PREFIX,
        }
    }

    #[must_use]
    pub const fn expected_sha256(self) -> &'static str {
        match self {
            Self::VoiceServer => VOICE_SERVER_DLL_SHA256,
        }
    }

    #[must_use]
    pub const fn raw_len(self) -> usize {
        match self {
            Self::VoiceServer => VOICE_SERVER_DLL_RAW_LEN,
        }
    }

    #[must_use]
    pub const fn compressed_bytes(self) -> &'static [u8] {
        match self {
            Self::VoiceServer => EMBEDDED_VOICE_SERVER_DLL_ZST,
        }
    }
}

fn digest_to_hex(digest: [u8; 32]) -> String {
    let mut out: String = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn sha256_bytes_hex(bytes: &[u8]) -> String {
    let mut hasher: Sha256 = Sha256::new();
    hasher.update(bytes);
    let digest: [u8; 32] = hasher.finalize().into();
    digest_to_hex(digest)
}

fn sha256_file_hex(path: &Path) -> std::io::Result<String> {
    let mut file: File = File::open(path)?;
    let mut hasher: Sha256 = Sha256::new();
    let mut buf: [u8; SHA256_READ_BUFFER_SIZE] = [0u8; SHA256_READ_BUFFER_SIZE];
    loop {
        let n: usize = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest: [u8; 32] = hasher.finalize().into();
    Ok(digest_to_hex(digest))
}

fn strip_extended_prefix(path: &Path) -> PathBuf {
    let s: Cow<'_, str> = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

fn ensure_writable_dir(dir: &Path) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let abs: PathBuf = if dir.is_absolute() {
        strip_extended_prefix(dir)
    } else {
        strip_extended_prefix(&std::env::current_dir()?.join(dir))
    };
    let seq: u64 = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let probe: PathBuf = abs.join(format!(".write_probe_{}_{seq}.tmp", std::process::id()));
    let _file: File = File::create(&probe)?;
    let _ = fs::remove_file(&probe);
    Ok(abs)
}

/// Returns candidate cache directories for server-side embedded OEP DLL extraction in priority order:
/// 1. `%LOCALAPPDATA%\DH_Tool_JA\server_dlls`
/// 2. `%TEMP%\DH_Tool_JA\server_dlls`
/// 3. `./.dh_cache/server_dlls`
#[must_use]
pub fn server_dll_cache_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::with_capacity(3);
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let trimmed: &str = local_app_data.trim();
        if !trimmed.is_empty() {
            candidates.push(
                PathBuf::from(trimmed)
                    .join(SERVER_DLL_CACHE_APP_DIR)
                    .join(SERVER_DLL_CACHE_SUBDIR),
            );
        }
    }
    candidates.push(
        std::env::temp_dir()
            .join(SERVER_DLL_CACHE_APP_DIR)
            .join(SERVER_DLL_CACHE_SUBDIR),
    );
    candidates.push(PathBuf::from(SERVER_DLL_CACHE_FALLBACK_DIR).join(SERVER_DLL_CACHE_SUBDIR));
    candidates
}

fn resolve_server_dll_cache_root(override_root: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(custom) = override_root {
        return ensure_writable_dir(custom).map_err(|err: std::io::Error| {
            format!(
                "内蔵サーバーDLLキャッシュディレクトリ作成に失敗しました ({}): {err}",
                custom.display()
            )
        });
    }
    for candidate in server_dll_cache_candidates() {
        if let Ok(abs) = ensure_writable_dir(&candidate) {
            return Ok(abs);
        }
    }
    Err("書き込み可能な内蔵サーバーDLLキャッシュディレクトリが見つかりませんでした。".to_string())
}

fn is_valid_cached_file(path: &Path, expected_len: usize, expected_sha256: &str) -> bool {
    let Ok(meta): std::io::Result<fs::Metadata> = fs::metadata(path) else {
        return false;
    };
    let Ok(expected_len_u64): Result<u64, std::num::TryFromIntError> = u64::try_from(expected_len)
    else {
        return false;
    };
    if !meta.is_file() || meta.len() != expected_len_u64 {
        return false;
    }
    sha256_file_hex(path).is_ok_and(|actual: String| actual == expected_sha256)
}

/// Extracts (or verifies an existing cached copy of) an embedded server-side OEP DLL.
pub fn extract_server_dll(
    id: ServerBuiltinDllId,
    override_cache_root: Option<&Path>,
) -> Result<ExtractedServerDll, String> {
    let cache_root: PathBuf = resolve_server_dll_cache_root(override_cache_root)?;
    let expected_sha: &'static str = id.expected_sha256();
    let expected_len: usize = id.raw_len();
    let short_hash: &str = expected_sha.get(..12).unwrap_or(expected_sha);
    let slot_dir: PathBuf = cache_root.join(format!("{}{short_hash}", id.subdir_prefix()));
    fs::create_dir_all(&slot_dir).map_err(|err: std::io::Error| {
        format!(
            "内蔵サーバーDLL展開先ディレクトリ作成に失敗しました ({}): {err}",
            slot_dir.display()
        )
    })?;

    let target_path: PathBuf = slot_dir.join(id.file_name());
    let label: String = languages::mod_label(id.label_code()).to_string();

    if is_valid_cached_file(&target_path, expected_len, expected_sha) {
        return Ok(ExtractedServerDll {
            id,
            file_name: id.file_name().to_string(),
            label,
            path: target_path,
            sha256: expected_sha.to_string(),
            raw_bytes: expected_len,
            reused_cache: true,
        });
    }

    let decompressed: Vec<u8> =
        zstd::stream::decode_all(id.compressed_bytes()).map_err(|err: std::io::Error| {
            format!(
                "内蔵サーバーDLL ({}) の展開に失敗しました: {err}",
                id.file_name()
            )
        })?;
    if decompressed.len() != expected_len
        || decompressed.len() < 2
        || decompressed[0] != b'M'
        || decompressed[1] != b'Z'
    {
        return Err(format!(
            "内蔵サーバーDLL ({}) のPEヘッダまたはサイズが一致しません (expected={expected_len}, actual={})",
            id.file_name(),
            decompressed.len()
        ));
    }

    let actual_sha: String = sha256_bytes_hex(&decompressed);
    if actual_sha != expected_sha {
        return Err(format!(
            "内蔵サーバーDLL ({}) のSHA-256検証に失敗しました (expected={expected_sha}, actual={actual_sha})",
            id.file_name()
        ));
    }

    let seq: u64 = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_path: PathBuf = slot_dir.join(format!(
        ".{}.{}.{seq}.tmp",
        id.file_name(),
        std::process::id()
    ));
    let mut tmp_file: File = File::create(&tmp_path).map_err(|err: std::io::Error| {
        format!(
            "内蔵サーバーDLL一時ファイル作成に失敗しました ({}): {err}",
            tmp_path.display()
        )
    })?;
    if let Err(err) = tmp_file.write_all(&decompressed) {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "内蔵サーバーDLL書き込みに失敗しました ({}): {err}",
            tmp_path.display()
        ));
    }
    drop(tmp_file);

    if target_path.exists() {
        let _ = fs::remove_file(&target_path);
    }
    if let Err(rename_err) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        if !is_valid_cached_file(&target_path, expected_len, expected_sha) {
            return Err(format!(
                "内蔵サーバーDLL配置に失敗しました ({}): {rename_err}",
                target_path.display()
            ));
        }
    }

    Ok(ExtractedServerDll {
        id,
        file_name: id.file_name().to_string(),
        label,
        path: target_path,
        sha256: expected_sha.to_string(),
        raw_bytes: expected_len,
        reused_cache: false,
    })
}

/// Extracts and SHA-256 verifies all embedded server-side OEP DLLs required by the selected
/// client-cooperation Content MODs in deterministic order.
pub fn prepare_server_dll_plan(
    selected_content_numbers: &[u32],
    override_cache_root: Option<&Path>,
) -> Result<Vec<ExtractedServerDll>, String> {
    let mut seen: HashSet<ServerBuiltinDllId> = HashSet::new();
    let mut extracted: Vec<ExtractedServerDll> = Vec::new();

    for &num in selected_content_numbers {
        let Some(opt): Option<&'static ModMenuOption> = find_content_option_by_number(num) else {
            continue;
        };
        for &dll_id in opt.dlls {
            if !seen.insert(dll_id) {
                continue;
            }
            let item: ExtractedServerDll = extract_server_dll(dll_id, override_cache_root)?;
            extracted.push(item);
        }
    }

    Ok(extracted)
}
