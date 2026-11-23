use std::fs;
use std::path::Path;

use dh_server_tool::mods::build_injection_plan;
use dh_server_tool::server::patch_logger::{
    append_patch_runtime_log_at, reset_patch_runtime_log_at, PatchLogLevel,
};
use dh_server_tool::server::supervisor::{
    alternate_port, format_mod_loaded_line, format_spawned_process_line, resolve_clean_path,
};
use uuid::Uuid;

#[test]
fn overwrites_log_per_round_and_appends_formatted_entries() {
    let path = std::env::temp_dir().join(format!("dh_patch_log_test_{}.log", Uuid::new_v4()));
    let patches = build_injection_plan(&[1], 7893);

    reset_patch_runtime_log_at(&path, 7895, &patches);
    append_patch_runtime_log_at(&path, PatchLogLevel::Info, "コンソール連携.js", "hello");
    append_patch_runtime_log_at(&path, PatchLogLevel::Error, "接続保護_乱入防止.js", "boom");

    let first = fs::read_to_string(&path).expect("read first round log");
    assert!(first.contains("Port: 7895"));
    assert!(first.contains("コンソール連携.js"));
    assert!(first.contains("[INFO] [コンソール連携.js] hello"));
    assert!(first.contains("[ERROR] [接続保護_乱入防止.js] boom"));

    reset_patch_runtime_log_at(&path, 7897, &patches[..4]);
    let second = fs::read_to_string(&path).expect("read second round log");
    assert!(second.contains("Port: 7897"));
    assert!(!second.contains("Port: 7895"));
    assert!(!second.contains("hello"));

    let _ = fs::remove_file(&path);
}

#[test]
fn alternates_server_ports_symmetrically() {
    let ports = [7895, 7897];
    assert_eq!(alternate_port(ports, 7895), 7897);
    assert_eq!(alternate_port(ports, 7897), 7895);
}

#[test]
fn resolves_relative_shipping_executable_without_verbatim_prefix() {
    let base = Path::new(r"C:\Game\WindowsServer");
    let rel = Path::new("./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe");
    let resolved = resolve_clean_path(rel, base);
    let s = resolved.to_string_lossy();
    assert!(!s.starts_with(r"\\?\"));
    assert!(s.ends_with(r"DreadHunger\Binaries\Win64\DreadHungerServer-Win64-Shipping.exe"));
}

#[test]
fn formats_mod_loaded_line_with_light_green_prefix_and_light_blue_name() {
    let line = format_mod_loaded_line(1, 13, "Webコンソール連携", "コンソール連携.js");
    assert_eq!(
        line,
        "\x1b[92m   [1/13] 注入完了: \x1b[94mWebコンソール連携 (コンソール連携.js)"
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
