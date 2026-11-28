use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use dh_client_launcher::builtin_dlls::{
    builtin_cache_candidates, builtin_dll_catalog, extract_builtin_dll_to,
    extract_enabled_builtin_dlls_in, selected_builtin_ids,
};
use dh_client_launcher::consts::builtin_dlls::{
    BUILTIN_EOS_FIX_FILE_NAME, BUILTIN_VOICE_CLIENT_FILE_NAME,
};
use dh_client_launcher::structs::builtin_dlls::{
    BuiltinDllConfig, BuiltinDllId, BuiltinEosFixPlan,
};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let seq: u64 = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "dh_client_launcher_builtin_{prefix}_{}_{}",
            std::process::id(),
            seq
        ));
        let _ = fs::remove_dir_all(&dir);
        assert!(
            fs::create_dir_all(&dir).is_ok(),
            "create temp cache dir: {}",
            dir.display()
        );
        Self { path: dir }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn builtin_catalog_exposes_all_three_embedded_dlls_with_valid_digests() {
    let catalog = builtin_dll_catalog();
    assert_eq!(catalog.len(), 3);
    assert_eq!(catalog[0].id, "eos_fix_plan_a");
    assert_eq!(catalog[0].file_name, BUILTIN_EOS_FIX_FILE_NAME);
    assert_eq!(catalog[1].id, "eos_fix_plan_b");
    assert_eq!(catalog[1].file_name, BUILTIN_EOS_FIX_FILE_NAME);
    assert_eq!(catalog[2].id, "voice_chat");
    assert_eq!(catalog[2].file_name, BUILTIN_VOICE_CLIENT_FILE_NAME);

    for meta in &catalog {
        assert_eq!(meta.sha256.len(), 64);
        assert!(meta.raw_bytes > 4096);
    }
    assert_ne!(catalog[0].sha256, catalog[1].sha256);
}

#[test]
fn extract_builtin_dlls_verifies_pe_header_hits_cache_and_recovers_from_tampering() {
    let tmp: TempDirGuard = TempDirGuard::new("extract");

    let standard_first =
        extract_builtin_dll_to(tmp.path(), BuiltinDllId::EosFixPlanA).expect("extract Plan A");
    assert!(!standard_first.cache_hit);
    assert!(standard_first.path.is_file());
    assert_eq!(
        standard_first
            .path
            .file_name()
            .and_then(std::ffi::OsStr::to_str),
        Some(BUILTIN_EOS_FIX_FILE_NAME)
    );
    let raw_standard: Vec<u8> = fs::read(&standard_first.path).expect("read extracted Plan A");
    assert_eq!(&raw_standard[..2], b"MZ");

    let standard_cached =
        extract_builtin_dll_to(tmp.path(), BuiltinDllId::EosFixPlanA).expect("cache hit Plan A");
    assert!(standard_cached.cache_hit);
    assert_eq!(standard_cached.path, standard_first.path);

    // Plan B extracts to a distinct isolated directory while keeping the authentic `eos_fix.dll` name.
    let private_uri =
        extract_builtin_dll_to(tmp.path(), BuiltinDllId::EosFixPlanB).expect("extract Plan B");
    assert!(!private_uri.cache_hit);
    assert_ne!(private_uri.path, standard_first.path);
    assert_eq!(
        private_uri
            .path
            .file_name()
            .and_then(std::ffi::OsStr::to_str),
        Some(BUILTIN_EOS_FIX_FILE_NAME)
    );

    // Tamper with cached file -> must detect SHA-256 mismatch and re-extract cleanly.
    fs::write(&standard_first.path, b"corrupted").expect("corrupt cached dll");
    let standard_repaired =
        extract_builtin_dll_to(tmp.path(), BuiltinDllId::EosFixPlanA).expect("repair Plan A");
    assert!(!standard_repaired.cache_hit);
    let repaired_bytes: Vec<u8> = fs::read(&standard_repaired.path).expect("read repaired dll");
    assert_eq!(repaired_bytes, raw_standard);
}

#[test]
fn extract_enabled_builtin_dlls_preserves_canonical_injection_order() {
    let tmp: TempDirGuard = TempDirGuard::new("order");

    let standard_and_voice: BuiltinDllConfig = BuiltinDllConfig {
        eos_fix_plan: BuiltinEosFixPlan::PlanA,
        voice_chat: true,
    };
    let pair_result = extract_enabled_builtin_dlls_in(tmp.path(), &standard_and_voice)
        .expect("extract Plan A + Voice");
    assert_eq!(pair_result.len(), 2);
    assert_eq!(pair_result[0].id, BuiltinDllId::EosFixPlanA);
    assert_eq!(pair_result[1].id, BuiltinDllId::VoiceClient);

    let private_only: BuiltinDllConfig = BuiltinDllConfig {
        eos_fix_plan: BuiltinEosFixPlan::PlanB,
        voice_chat: false,
    };
    assert_eq!(
        selected_builtin_ids(&private_only),
        vec![BuiltinDllId::EosFixPlanB]
    );
    let single_result =
        extract_enabled_builtin_dlls_in(tmp.path(), &private_only).expect("extract Plan B only");
    assert_eq!(single_result.len(), 1);
    assert_eq!(single_result[0].id, BuiltinDllId::EosFixPlanB);

    let disabled_cfg: BuiltinDllConfig = BuiltinDllConfig {
        eos_fix_plan: BuiltinEosFixPlan::None,
        voice_chat: false,
    };
    let empty_result =
        extract_enabled_builtin_dlls_in(tmp.path(), &disabled_cfg).expect("extract none");
    assert_eq!(empty_result.len(), 0);
}

#[test]
fn builtin_cache_candidates_include_fallback_under_base_dir() {
    let tmp: TempDirGuard = TempDirGuard::new("candidates");
    let candidates: Vec<PathBuf> = builtin_cache_candidates(tmp.path());
    assert!(candidates.len() >= 2);
    assert_eq!(
        candidates.last().map(PathBuf::as_path),
        Some(tmp.path().join(".dh_client_cache").as_path())
    );
}
