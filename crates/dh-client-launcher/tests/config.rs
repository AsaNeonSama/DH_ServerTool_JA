use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use dh_client_launcher::config::{
    describe_launch_mode, evaluate_file_statuses, load_or_create_config_in, normalize_display_path,
    resolve_entry_path, save_config_in,
};
use dh_client_launcher::consts::app::APP_LICENSE_TEXT;
use dh_client_launcher::consts::config::{CONFIG_FILE_NAME, DEFAULT_EXE_RELATIVE_PATH};
use dh_client_launcher::structs::builtin_dlls::{BuiltinDllConfig, BuiltinEosFixPlan};
use dh_client_launcher::structs::config::LauncherConfig;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let seq: u64 = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "dh_client_launcher_{prefix}_{}_{}",
            std::process::id(),
            seq
        ));
        let _ = fs::remove_dir_all(&dir);
        assert!(
            fs::create_dir_all(&dir).is_ok(),
            "create temp test dir: {}",
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

fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        assert!(
            fs::create_dir_all(parent).is_ok(),
            "create parent dirs: {}",
            parent.display()
        );
    }
    assert!(
        fs::write(path, b"stub").is_ok(),
        "write stub file: {}",
        path.display()
    );
}

#[test]
fn default_config_uses_relative_exe_path_and_builtin_defaults() {
    let cfg: LauncherConfig = LauncherConfig::default();
    assert_eq!(cfg.exe_path, DEFAULT_EXE_RELATIVE_PATH);
    assert_eq!(cfg.builtin.eos_fix_plan, BuiltinEosFixPlan::PlanB);
    assert!(!cfg.builtin.voice_chat);
}

#[test]
fn load_or_create_config_creates_file_with_builtin_toggles() {
    let tmp: TempDirGuard = TempDirGuard::new("cfg_autocreate");

    let cfg: LauncherConfig = load_or_create_config_in(tmp.path()).expect("load or create config");

    assert!(tmp.path().join(CONFIG_FILE_NAME).is_file());
    assert_eq!(cfg.exe_path, DEFAULT_EXE_RELATIVE_PATH);
    assert_eq!(cfg.builtin.eos_fix_plan, BuiltinEosFixPlan::PlanB);
    assert!(!cfg.builtin.voice_chat);
}

#[test]
fn load_or_create_config_ignores_legacy_launch_args_and_external_fields() {
    let tmp: TempDirGuard = TempDirGuard::new("cfg_legacy");
    let legacy_json: &str = r#"{
  "exePath": "./DreadHunger-Win64-Shipping.exe",
  "launchArgs": "-dx11 -windowed",
  "builtin": {
    "eosFixPlan": "plan_a",
    "voiceChat": true
  },
  "dlls": [
    {
      "path": "./custom.dll",
      "name": "custom.dll",
      "enabled": true
    }
  ],
  "scripts": [
    {
      "path": "./client_scripts/hud.js",
      "name": "hud.js",
      "enabled": true
    }
  ],
  "jsScripts": ["./legacy.js"]
}"#;
    fs::write(tmp.path().join(CONFIG_FILE_NAME), legacy_json).expect("write legacy json");

    let parsed: LauncherConfig =
        load_or_create_config_in(tmp.path()).expect("legacy config must deserialize cleanly");
    assert_eq!(parsed.exe_path, "./DreadHunger-Win64-Shipping.exe");
    assert_eq!(parsed.builtin.eos_fix_plan, BuiltinEosFixPlan::PlanA);
    assert!(parsed.builtin.voice_chat);

    let minimal_tmp: TempDirGuard = TempDirGuard::new("cfg_minimal");
    fs::write(
        minimal_tmp.path().join(CONFIG_FILE_NAME),
        r#"{"exePath": "./game.exe"}"#,
    )
    .expect("write minimal json");
    let minimal: LauncherConfig =
        load_or_create_config_in(minimal_tmp.path()).expect("minimal config must apply defaults");
    assert_eq!(minimal.builtin.eos_fix_plan, BuiltinEosFixPlan::PlanB);
    assert!(!minimal.builtin.voice_chat);
}

#[test]
fn save_and_reload_config_roundtrip_preserves_builtin_fields_without_removed_keys() {
    let tmp: TempDirGuard = TempDirGuard::new("cfg_roundtrip");
    let original: LauncherConfig = LauncherConfig {
        exe_path: "./DreadHunger-Win64-Shipping.exe".to_string(),
        builtin: BuiltinDllConfig {
            eos_fix_plan: BuiltinEosFixPlan::PlanA,
            voice_chat: true,
        },
    };

    save_config_in(tmp.path(), &original).expect("save config");
    let raw: String =
        fs::read_to_string(tmp.path().join(CONFIG_FILE_NAME)).expect("read saved config");
    assert!(!raw.contains("launchArgs"));
    assert!(!raw.contains("\"dlls\""));
    assert!(!raw.contains("\"scripts\""));

    let reloaded: LauncherConfig = load_or_create_config_in(tmp.path()).expect("reload config");
    assert_eq!(reloaded, original);
}

#[test]
fn evaluate_file_statuses_tracks_existing_and_missing_exe() {
    let tmp: TempDirGuard = TempDirGuard::new("cfg_statuses");
    let cfg: LauncherConfig = LauncherConfig::default();

    let missing = evaluate_file_statuses(tmp.path(), &cfg);
    assert!(!missing.exe_exists);

    touch(
        &tmp.path()
            .join("DreadHunger/Binaries/Win64/DreadHunger-Win64-Shipping.exe"),
    );
    let present = evaluate_file_statuses(tmp.path(), &cfg);
    assert!(present.exe_exists);
}

#[test]
fn normalize_display_path_returns_relative_for_children() {
    let tmp: TempDirGuard = TempDirGuard::new("cfg_norm_path");
    let child: PathBuf = tmp
        .path()
        .join("DreadHunger")
        .join("Binaries")
        .join("Win64")
        .join("DreadHunger-Win64-Shipping.exe");
    touch(&child);

    let normalized: String = normalize_display_path(tmp.path(), &child);
    assert_eq!(
        normalized,
        "./DreadHunger/Binaries/Win64/DreadHunger-Win64-Shipping.exe"
    );
}

#[test]
fn resolve_entry_path_strips_curdir_and_normalizes_separators() {
    let base: PathBuf = PathBuf::from(r"D:\SteamLibrary\steamapps\common\Dread Hunger");
    let resolved: PathBuf = resolve_entry_path(&base, DEFAULT_EXE_RELATIVE_PATH);
    let rendered: String = resolved.display().to_string();
    assert_eq!(
        rendered,
        r"D:\SteamLibrary\steamapps\common\Dread Hunger\DreadHunger\Binaries\Win64\DreadHunger-Win64-Shipping.exe"
    );
    assert!(!rendered.contains("./"));
    assert!(!rendered.contains('/'));

    let parent_collapsed: PathBuf =
        resolve_entry_path(&base, "./DreadHunger/Binaries/../Binaries/Win64/game.exe");
    assert_eq!(
        parent_collapsed.display().to_string(),
        r"D:\SteamLibrary\steamapps\common\Dread Hunger\DreadHunger\Binaries\Win64\game.exe"
    );
}

#[test]
fn describe_launch_mode_covers_all_modes() {
    assert!(describe_launch_mode(0, 0).contains("通常起動"));
    assert!(describe_launch_mode(2, 0).contains("内蔵DLL"));
    assert!(describe_launch_mode(2, 0).contains("Frida未使用"));
    assert!(describe_launch_mode(0, 1).contains("内蔵JS"));
    assert!(describe_launch_mode(2, 1).contains("内蔵DLL"));
    assert!(describe_launch_mode(2, 1).contains("内蔵JS"));
}

#[test]
fn embedded_license_text_is_present() {
    assert!(APP_LICENSE_TEXT.len() > 1000);
    assert!(APP_LICENSE_TEXT.contains("DH_Tool_JA"));
    assert!(APP_LICENSE_TEXT.contains("日本語"));
}
