use std::fs;
use std::path::{Path, PathBuf};

use dh_server_tool::mods::{build_injection_plan, ModGroup};
use dh_server_tool::server::patch_logger::{
    append_patch_runtime_log_at, start_patch_runtime_log_in, PatchLogLevel,
    MAX_PATCH_RUNTIME_LOG_FILES, PATCH_RUNTIME_LOG_EXTENSION, PATCH_RUNTIME_LOG_PREFIX,
};
use dh_server_tool::server::supervisor::{
    alternate_port, format_mod_loaded_line, format_mod_loaded_line_for_group,
    format_spawned_process_line, next_server_port, resolve_clean_path,
};
use uuid::Uuid;

#[test]
fn creates_timestamped_logs_keeps_newest_three_and_preserves_unrelated_files() {
    let dir: PathBuf = std::env::temp_dir().join(format!("dh_patch_log_dir_{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("create temp log dir");
    let patches = build_injection_plan(&[1], 7893);

    // Seed unrelated .log files that must NEVER be deleted.
    let unrelated_other: PathBuf = dir.join("other.log");
    let unrelated_notes: PathBuf = dir.join("patch_runtime_notes.log");
    fs::write(&unrelated_other, "unrelated log").expect("write other.log");
    fs::write(&unrelated_notes, "notes log").expect("write patch_runtime_notes.log");

    // Create 5 sequential match logs (also exercises same-second disambiguation).
    let mut created: Vec<PathBuf> = Vec::new();
    for i in 0u16..5u16 {
        let port: u16 = 7895 + i;
        let p: PathBuf = start_patch_runtime_log_in(&dir, port, &patches);
        let name: &str = p
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf-8 filename");
        assert!(name.starts_with(PATCH_RUNTIME_LOG_PREFIX));
        assert!(name.ends_with(PATCH_RUNTIME_LOG_EXTENSION));
        append_patch_runtime_log_at(
            &p,
            PatchLogLevel::Info,
            "コンソール連携.js",
            &format!("round_{i}"),
        );
        append_patch_runtime_log_at(
            &p,
            PatchLogLevel::Error,
            "接続保護_乱入防止.js",
            &format!("err_{i}"),
        );
        created.push(p);
    }

    // Oldest 2 deleted; newest 3 intact with correct headers and appended entries.
    assert!(!created[0].exists(), "oldest match #0 must be pruned");
    assert!(!created[1].exists(), "oldest match #1 must be pruned");
    for (idx, path) in created.iter().enumerate().skip(2) {
        assert!(path.exists(), "recent match #{idx} must be kept");
        let content: String = fs::read_to_string(path).expect("read kept match log");
        let expected_port: u16 = 7895 + u16::try_from(idx).unwrap();
        assert!(content.contains(&format!("Port: {expected_port}")));
        assert!(content.contains("コンソール連携.js"));
        assert!(content.contains(&format!("[INFO] [コンソール連携.js] round_{idx}")));
        assert!(content.contains(&format!("[ERROR] [接続保護_乱入防止.js] err_{idx}")));
    }

    // Distinct filenames and exact 3 matching patch_runtime_*.log files remaining.
    let kept_matching: Vec<String> = fs::read_dir(&dir)
        .expect("read dir")
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| {
            name.starts_with(PATCH_RUNTIME_LOG_PREFIX)
                && name.ends_with(PATCH_RUNTIME_LOG_EXTENSION)
                && name != "patch_runtime_notes.log"
        })
        .collect();
    assert_eq!(kept_matching.len(), MAX_PATCH_RUNTIME_LOG_FILES);
    assert!(unrelated_other.exists(), "other.log must never be deleted");
    assert!(
        unrelated_notes.exists(),
        "patch_runtime_notes.log must never be deleted"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn prunes_older_timestamps_chronologically_across_dates() {
    let dir: PathBuf = std::env::temp_dir().join(format!("dh_patch_log_dates_{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("create temp log dir");
    let patches = build_injection_plan(&[], 7893);

    let old1: PathBuf = dir.join("patch_runtime_20250101_000001.log");
    let old2: PathBuf = dir.join("patch_runtime_20250101_000002.log");
    let old3: PathBuf = dir.join("patch_runtime_20250102_120000.log");
    let old4: PathBuf = dir.join("patch_runtime_20250102_120000_01.log");
    for p in [&old1, &old2, &old3, &old4] {
        fs::write(p, "historical").expect("seed old log");
    }

    let newest: PathBuf = start_patch_runtime_log_in(&dir, 7895, &patches);
    assert!(!old1.exists());
    assert!(!old2.exists());
    assert!(old3.exists(), "3rd newest timestamped log must remain");
    assert!(old4.exists(), "2nd newest timestamped log must remain");
    assert!(newest.exists(), "newly created launch log must remain");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn alternates_server_ports_symmetrically() {
    let ports = [7895, 7897];
    assert_eq!(alternate_port(ports, 7895), 7897);
    assert_eq!(alternate_port(ports, 7897), 7895);
}

#[test]
fn next_server_port_respects_switch_port_on_restart_flag() {
    let ports = [7895, 7897];
    assert_eq!(next_server_port(ports, 7895, true), 7897);
    assert_eq!(next_server_port(ports, 7897, true), 7895);

    assert_eq!(next_server_port(ports, 7895, false), 7895);
    assert_eq!(next_server_port([7895, 7895], 7895, false), 7895);
    assert_eq!(next_server_port([9001, 9001], 9001, false), 9001);
}

#[test]
fn resolves_relative_shipping_executable_without_verbatim_prefix() {
    let base = Path::new(r"C:\Game\WindowsServer");
    let rel = Path::new("./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe");
    let resolved = resolve_clean_path(rel, base);
    let s = resolved.to_string_lossy();
    assert!(!s.starts_with(r"\\?\"));
    assert!(s.ends_with(r"DreadHunger\Binaries\Win64\DreadHungerServer-Win64-Shipping.exe"));

    let verbatim_base = Path::new(r"\\?\C:\Game\WindowsServer");
    let stripped = resolve_clean_path(rel, verbatim_base);
    assert!(!stripped.to_string_lossy().starts_with(r"\\?\"));
}

#[test]
fn formats_mod_loaded_line_with_light_green_prefix_and_light_blue_name() {
    let line = format_mod_loaded_line(1, 13, "Webコンソール連携", "コンソール連携.js");
    assert_eq!(
        line,
        "\x1b[92m   [1/13] 注入完了: \x1b[94mWebコンソール連携 (コンソール連携.js)"
    );

    let content_line = format_mod_loaded_line_for_group(
        5,
        6,
        "カスタムスペル(新呪文5種)",
        "カスタムスペル.js",
        ModGroup::ContentMod,
    );
    assert_eq!(
        content_line,
        "\x1b[92m   [5/6] 注入完了: \x1b[95mカスタムスペル(新呪文5種) (カスタムスペル.js)"
    );
}

#[test]
fn formats_spawned_process_line_with_light_yellow_and_white_pid() {
    let line = format_spawned_process_line(12345);
    assert_eq!(
        line,
        "\x1b[93m * プロセス生成完了 (PID: \x1b[97m12345\x1b[93m)"
    );
}
