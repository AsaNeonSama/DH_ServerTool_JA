use std::borrow::Cow;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::consts::builtin_dlls::{
    BUILTIN_CACHE_APP_DIR, BUILTIN_CACHE_FALLBACK_DIR, BUILTIN_CACHE_SUBDIR,
    BUILTIN_EOS_FIX_FILE_NAME, BUILTIN_EOS_PLAN_A_ID, BUILTIN_EOS_PLAN_A_LABEL,
    BUILTIN_EOS_PLAN_A_SUBDIR_PREFIX, BUILTIN_EOS_PLAN_B_ID, BUILTIN_EOS_PLAN_B_LABEL,
    BUILTIN_EOS_PLAN_B_SUBDIR_PREFIX, BUILTIN_VOICE_CLIENT_FILE_NAME, BUILTIN_VOICE_ID,
    BUILTIN_VOICE_LABEL, BUILTIN_VOICE_SUBDIR_PREFIX, EMBEDDED_EOS_FIX_PLAN_A_ZST,
    EMBEDDED_EOS_FIX_PLAN_B_ZST, EMBEDDED_VOICE_CLIENT_ZST, EOS_FIX_PLAN_A_RAW_LEN,
    EOS_FIX_PLAN_A_SHA256, EOS_FIX_PLAN_B_RAW_LEN, EOS_FIX_PLAN_B_SHA256, SHA256_READ_BUFFER_SIZE,
    VOICE_CLIENT_RAW_LEN, VOICE_CLIENT_SHA256,
};
use crate::consts::discovery::{
    EOS_PLAN_A_DESCRIPTION, EOS_PLAN_B_DESCRIPTION, VOICE_CLIENT_DESCRIPTION,
};
use crate::structs::builtin_dlls::{
    BuiltinDllConfig, BuiltinDllId, BuiltinDllMeta, BuiltinEosFixPlan, ExtractedBuiltinDll,
};

static WRITE_SEQ: AtomicU64 = AtomicU64::new(1);

impl Default for BuiltinDllConfig {
    fn default() -> Self {
        Self {
            eos_fix_plan: BuiltinEosFixPlan::PlanB,
            voice_chat: false,
        }
    }
}

impl BuiltinDllId {
    #[must_use]
    pub const fn id_str(self) -> &'static str {
        match self {
            Self::EosFixPlanA => BUILTIN_EOS_PLAN_A_ID,
            Self::EosFixPlanB => BUILTIN_EOS_PLAN_B_ID,
            Self::VoiceClient => BUILTIN_VOICE_ID,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::EosFixPlanA => BUILTIN_EOS_PLAN_A_LABEL,
            Self::EosFixPlanB => BUILTIN_EOS_PLAN_B_LABEL,
            Self::VoiceClient => BUILTIN_VOICE_LABEL,
        }
    }

    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::EosFixPlanA => EOS_PLAN_A_DESCRIPTION,
            Self::EosFixPlanB => EOS_PLAN_B_DESCRIPTION,
            Self::VoiceClient => VOICE_CLIENT_DESCRIPTION,
        }
    }

    #[must_use]
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::EosFixPlanA | Self::EosFixPlanB => BUILTIN_EOS_FIX_FILE_NAME,
            Self::VoiceClient => BUILTIN_VOICE_CLIENT_FILE_NAME,
        }
    }

    #[must_use]
    pub const fn subdir_prefix(self) -> &'static str {
        match self {
            Self::EosFixPlanA => BUILTIN_EOS_PLAN_A_SUBDIR_PREFIX,
            Self::EosFixPlanB => BUILTIN_EOS_PLAN_B_SUBDIR_PREFIX,
            Self::VoiceClient => BUILTIN_VOICE_SUBDIR_PREFIX,
        }
    }

    #[must_use]
    pub const fn expected_sha256(self) -> &'static str {
        match self {
            Self::EosFixPlanA => EOS_FIX_PLAN_A_SHA256,
            Self::EosFixPlanB => EOS_FIX_PLAN_B_SHA256,
            Self::VoiceClient => VOICE_CLIENT_SHA256,
        }
    }

    #[must_use]
    pub const fn raw_len(self) -> usize {
        match self {
            Self::EosFixPlanA => EOS_FIX_PLAN_A_RAW_LEN,
            Self::EosFixPlanB => EOS_FIX_PLAN_B_RAW_LEN,
            Self::VoiceClient => VOICE_CLIENT_RAW_LEN,
        }
    }

    #[must_use]
    pub const fn compressed_bytes(self) -> &'static [u8] {
        match self {
            Self::EosFixPlanA => EMBEDDED_EOS_FIX_PLAN_A_ZST,
            Self::EosFixPlanB => EMBEDDED_EOS_FIX_PLAN_B_ZST,
            Self::VoiceClient => EMBEDDED_VOICE_CLIENT_ZST,
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

/// Returns candidate cache directories for built-in client DLL extraction in priority order:
/// 1. `%LOCALAPPDATA%\DH_ClientLauncher_JA\builtin_dlls`
/// 2. `%TEMP%\DH_ClientLauncher_JA\builtin_dlls`
/// 3. `<base_dir>\.dh_client_cache`
#[must_use]
pub fn builtin_cache_candidates(base_dir: &Path) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::with_capacity(3);
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let trimmed: &str = local_app_data.trim();
        if !trimmed.is_empty() {
            candidates.push(
                PathBuf::from(trimmed)
                    .join(BUILTIN_CACHE_APP_DIR)
                    .join(BUILTIN_CACHE_SUBDIR),
            );
        }
    }
    candidates.push(
        std::env::temp_dir()
            .join(BUILTIN_CACHE_APP_DIR)
            .join(BUILTIN_CACHE_SUBDIR),
    );
    candidates.push(base_dir.join(BUILTIN_CACHE_FALLBACK_DIR));
    candidates
}

/// Returns the ordered list of enabled built-in client DLLs (EOS Fix first, then Voice Chat).
#[must_use]
pub fn selected_builtin_ids(config: &BuiltinDllConfig) -> Vec<BuiltinDllId> {
    let mut ids: Vec<BuiltinDllId> = Vec::with_capacity(2);
    match config.eos_fix_plan {
        BuiltinEosFixPlan::None => {}
        BuiltinEosFixPlan::PlanA => ids.push(BuiltinDllId::EosFixPlanA),
        BuiltinEosFixPlan::PlanB => ids.push(BuiltinDllId::EosFixPlanB),
    }
    if config.voice_chat {
        ids.push(BuiltinDllId::VoiceClient);
    }
    ids
}

/// Returns read-only metadata for all 3 embedded built-in client DLLs.
#[must_use]
pub fn builtin_dll_catalog() -> Vec<BuiltinDllMeta> {
    let all: [BuiltinDllId; 3] = [
        BuiltinDllId::EosFixPlanA,
        BuiltinDllId::EosFixPlanB,
        BuiltinDllId::VoiceClient,
    ];
    all.into_iter()
        .map(|id: BuiltinDllId| BuiltinDllMeta {
            id: id.id_str().to_string(),
            label: id.label().to_string(),
            file_name: id.file_name().to_string(),
            description: id.description().to_string(),
            sha256: id.expected_sha256().to_string(),
            raw_bytes: id.raw_len(),
        })
        .collect()
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

/// Extracts (or verifies an already cached copy of) a single built-in client DLL inside `cache_root`.
pub fn extract_builtin_dll_to(
    cache_root: &Path,
    id: BuiltinDllId,
) -> Result<ExtractedBuiltinDll, String> {
    let expected_sha: &'static str = id.expected_sha256();
    let expected_len: usize = id.raw_len();
    let short_hash: &str = expected_sha.get(..12).unwrap_or(expected_sha);
    let slot_dir: PathBuf = cache_root.join(format!("{}{short_hash}", id.subdir_prefix()));
    fs::create_dir_all(&slot_dir).map_err(|err: std::io::Error| {
        format!(
            "内蔵DLLキャッシュディレクトリ作成に失敗しました ({}): {err}",
            slot_dir.display()
        )
    })?;

    let target_path: PathBuf = strip_extended_prefix(&slot_dir.join(id.file_name()));
    if is_valid_cached_file(&target_path, expected_len, expected_sha) {
        return Ok(ExtractedBuiltinDll {
            id,
            label: id.label(),
            path: target_path,
            sha256: expected_sha,
            cache_hit: true,
        });
    }

    let raw_bytes: Vec<u8> =
        zstd::stream::decode_all(id.compressed_bytes()).map_err(|err: std::io::Error| {
            format!("内蔵DLL ({}) の展開に失敗しました: {err}", id.label())
        })?;
    if raw_bytes.len() != expected_len {
        return Err(format!(
            "内蔵DLL ({}) のサイズ検証に失敗しました: 期待 {} B / 実際 {} B",
            id.label(),
            expected_len,
            raw_bytes.len()
        ));
    }
    let actual_sha: String = sha256_bytes_hex(&raw_bytes);
    if actual_sha != expected_sha {
        return Err(format!(
            "内蔵DLL ({}) のSHA-256検証に失敗しました: 期待 {expected_sha} / 実際 {actual_sha}",
            id.label()
        ));
    }

    let seq: u64 = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_path: PathBuf = slot_dir.join(format!(
        "{}.{}.{seq}.tmp",
        id.file_name(),
        std::process::id()
    ));

    let write_res: std::io::Result<()> = (|| -> std::io::Result<()> {
        let mut file: File = File::create(&tmp_path)?;
        file.write_all(&raw_bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(err) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(format!(
            "内蔵DLL ({}) の書込に失敗しました: {err}",
            id.label()
        ));
    }

    if target_path.exists() {
        let _ = fs::remove_file(&target_path);
    }
    if let Err(rename_err) = fs::rename(&tmp_path, &target_path) {
        let _ = fs::remove_file(&tmp_path);
        if !is_valid_cached_file(&target_path, expected_len, expected_sha) {
            return Err(format!(
                "内蔵DLL ({}) の配置に失敗しました: {rename_err}",
                id.label()
            ));
        }
    }

    if !is_valid_cached_file(&target_path, expected_len, expected_sha) {
        return Err(format!(
            "内蔵DLL ({}) の配置後SHA-256検証に失敗しました",
            id.label()
        ));
    }

    Ok(ExtractedBuiltinDll {
        id,
        label: id.label(),
        path: target_path,
        sha256: expected_sha,
        cache_hit: false,
    })
}

/// Extracts all enabled built-in client DLLs in canonical order into `cache_root`.
pub fn extract_enabled_builtin_dlls_in(
    cache_root: &Path,
    config: &BuiltinDllConfig,
) -> Result<Vec<ExtractedBuiltinDll>, String> {
    let ids: Vec<BuiltinDllId> = selected_builtin_ids(config);
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let abs_root: PathBuf = ensure_writable_dir(cache_root).map_err(|err: std::io::Error| {
        format!(
            "内蔵DLLキャッシュ先 ({}) に書き込めません: {err}",
            cache_root.display()
        )
    })?;

    let mut extracted: Vec<ExtractedBuiltinDll> = Vec::with_capacity(ids.len());
    for id in ids {
        extracted.push(extract_builtin_dll_to(&abs_root, id)?);
    }
    Ok(extracted)
}

/// Extracts all enabled built-in client DLLs using the priority cache candidate list.
pub fn extract_enabled_builtin_dlls(
    base_dir: &Path,
    config: &BuiltinDllConfig,
) -> Result<Vec<ExtractedBuiltinDll>, String> {
    let ids: Vec<BuiltinDllId> = selected_builtin_ids(config);
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut chosen_root: Option<PathBuf> = None;
    for candidate in builtin_cache_candidates(base_dir) {
        if let Ok(abs) = ensure_writable_dir(&candidate) {
            chosen_root = Some(abs);
            break;
        }
    }
    let Some(root) = chosen_root else {
        return Err(
            "内蔵DLLを展開できる書込可能なキャッシュディレクトリが見つかりませんでした。"
                .to_string(),
        );
    };

    let mut extracted: Vec<ExtractedBuiltinDll> = Vec::with_capacity(ids.len());
    for id in ids {
        extracted.push(extract_builtin_dll_to(&root, id)?);
    }
    Ok(extracted)
}
