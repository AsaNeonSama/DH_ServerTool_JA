use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use dh_client_launcher::consts::app::{
    DREAD_HUNGER_STEAM_APP_ID, ENV_STEAM_APP_ID, ENV_STEAM_GAME_ID, MAX_BUFFERED_LOGS,
};
use dh_client_launcher::consts::events::{CATEGORY_DLL, CATEGORY_SYS, LEVEL_INFO, LEVEL_OK};
use dh_client_launcher::launcher::{
    clear_buffered_logs, count_enabled_builtins, make_log_event, prepare_launch_inputs,
    push_buffered_log, snapshot_buffered_logs, steam_launch_env_overrides,
};
use dh_client_launcher::structs::builtin_dlls::{
    BuiltinDllConfig, BuiltinDllId, BuiltinEosFixPlan,
};
use dh_client_launcher::structs::config::LauncherConfig;
use dh_client_launcher::structs::state::LogBuffer;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let seq: u64 = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "dh_client_launcher_launch_{prefix}_{}_{}",
            std::process::id(),
            seq
        ));
        let _ = fs::remove_dir_all(&dir);
        assert!(
            fs::create_dir_all(&dir).is_ok(),
            "create temp dir: {}",
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

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        assert!(
            fs::create_dir_all(parent).is_ok(),
            "create parent dirs: {}",
            parent.display()
        );
    }
    assert!(
        fs::write(path, content).is_ok(),
        "write file: {}",
        path.display()
    );
}

#[test]
fn steam_launch_env_overrides_sets_dread_hunger_app_and_game_id() {
    let overrides: Vec<(String, String)> = steam_launch_env_overrides();
    assert_eq!(
        overrides,
        vec![
            (
                ENV_STEAM_APP_ID.to_string(),
                DREAD_HUNGER_STEAM_APP_ID.to_string(),
            ),
            (
                ENV_STEAM_GAME_ID.to_string(),
                DREAD_HUNGER_STEAM_APP_ID.to_string(),
            ),
        ]
    );
    assert_eq!(DREAD_HUNGER_STEAM_APP_ID, "1418630");
}

#[test]
fn prepare_launch_inputs_rejects_empty_and_missing_exe() {
    let tmp: TempDirGuard = TempDirGuard::new("missing");
    let empty_exe_cfg: LauncherConfig = LauncherConfig {
        exe_path: "   ".to_string(),
        builtin: BuiltinDllConfig {
            eos_fix_plan: BuiltinEosFixPlan::None,
            voice_chat: false,
        },
    };
    assert!(prepare_launch_inputs(tmp.path(), &empty_exe_cfg).is_err());

    let missing_exe_cfg: LauncherConfig = LauncherConfig::default();
    assert!(prepare_launch_inputs(tmp.path(), &missing_exe_cfg).is_err());
}

#[test]
fn prepare_launch_inputs_uses_default_empty_args_steam_env_and_zero_frida_mods() {
    let tmp: TempDirGuard = TempDirGuard::new("default_launch");
    write_file(&tmp.path().join("game.exe"), "MZ");

    let cfg: LauncherConfig = LauncherConfig {
        exe_path: "./game.exe".to_string(),
        builtin: BuiltinDllConfig::default(),
    };

    let prepared = prepare_launch_inputs(tmp.path(), &cfg).expect("prepare launch inputs");

    assert!(prepared.exe_path.ends_with("game.exe"));
    assert_eq!(prepared.args, Vec::<String>::new());
    assert_eq!(prepared.mods.len(), 0);
    assert_eq!(prepared.builtin_ids, vec![BuiltinDllId::EosFixPlanB]);
    assert_eq!(prepared.env_overrides, steam_launch_env_overrides());
    assert!(prepared.mode_label.contains("内蔵DLL"));
    assert!(prepared.mode_label.contains("1件"));
    assert!(prepared.mode_label.contains("Frida未使用"));
}

#[test]
fn prepare_launch_inputs_supports_plain_mode_when_all_builtins_disabled() {
    let tmp: TempDirGuard = TempDirGuard::new("plain_launch");
    write_file(&tmp.path().join("game.exe"), "MZ");

    let cfg: LauncherConfig = LauncherConfig {
        exe_path: "./game.exe".to_string(),
        builtin: BuiltinDllConfig {
            eos_fix_plan: BuiltinEosFixPlan::None,
            voice_chat: false,
        },
    };

    let prepared = prepare_launch_inputs(tmp.path(), &cfg).expect("prepare plain launch");
    assert_eq!(prepared.args, Vec::<String>::new());
    assert_eq!(prepared.builtin_ids, Vec::<BuiltinDllId>::new());
    assert_eq!(prepared.mods.len(), 0);
    assert_eq!(prepared.env_overrides, steam_launch_env_overrides());
    assert!(prepared.mode_label.contains("通常起動"));
}

#[test]
fn count_enabled_builtins_and_log_event_helpers_work() {
    let default_cfg: LauncherConfig = LauncherConfig::default();
    assert_eq!(count_enabled_builtins(&default_cfg), 1);

    let pair_cfg: LauncherConfig = LauncherConfig {
        exe_path: "./game.exe".to_string(),
        builtin: BuiltinDllConfig {
            eos_fix_plan: BuiltinEosFixPlan::PlanA,
            voice_chat: true,
        },
    };
    assert_eq!(count_enabled_builtins(&pair_cfg), 2);

    let none_cfg: LauncherConfig = LauncherConfig {
        exe_path: "./game.exe".to_string(),
        builtin: BuiltinDllConfig {
            eos_fix_plan: BuiltinEosFixPlan::None,
            voice_chat: false,
        },
    };
    assert_eq!(count_enabled_builtins(&none_cfg), 0);

    let ev = make_log_event(LEVEL_OK, CATEGORY_DLL, "eos_fix.dll", "loaded");
    assert_eq!(ev.level, LEVEL_OK);
    assert_eq!(ev.category, CATEGORY_DLL);
    assert_eq!(ev.source, "eos_fix.dll");
    assert_eq!(ev.message, "loaded");
    assert_eq!(ev.timestamp.len(), 12);
}

#[test]
fn buffered_logs_preserve_order_enforce_ring_cap_and_clear() {
    let buffer: LogBuffer = Arc::new(Mutex::new(VecDeque::new()));
    let total: usize = MAX_BUFFERED_LOGS + 15;
    for idx in 0..total {
        push_buffered_log(
            &buffer,
            make_log_event(LEVEL_INFO, CATEGORY_SYS, "test", format!("msg-{idx}")),
        );
    }

    let snap = snapshot_buffered_logs(&buffer);
    assert_eq!(snap.len(), MAX_BUFFERED_LOGS);
    assert_eq!(snap[0].message, "msg-15");
    assert_eq!(
        snap[MAX_BUFFERED_LOGS - 1].message,
        format!("msg-{}", total - 1)
    );

    clear_buffered_logs(&buffer);
    assert_eq!(snapshot_buffered_logs(&buffer).len(), 0);
}
