use std::env;
use std::fs;
use std::path::PathBuf;

use dh_server_tool::config::{
    load_or_create_config_at, parse_config_value, AppConfig, ConfigLoadOutcome,
};
use serde_json::Value;
use uuid::Uuid;

fn temp_config_path() -> PathBuf {
    env::temp_dir().join(format!("dh_config_test_{}.json", Uuid::new_v4()))
}

#[test]
fn creates_default_config_when_missing() {
    let path = temp_config_path();
    assert!(!path.exists());

    let loaded = load_or_create_config_at(&path);
    assert_eq!(loaded.outcome, ConfigLoadOutcome::CreatedDefault);
    assert_eq!(loaded.config, AppConfig::default());
    assert!(path.exists());

    let reloaded = load_or_create_config_at(&path);
    assert_eq!(reloaded.outcome, ConfigLoadOutcome::Loaded);
    assert_eq!(reloaded.config, AppConfig::default());

    let _ = fs::remove_file(&path);
}

#[test]
fn falls_back_field_by_field_for_invalid_values() {
    let raw: Value = serde_json::json!({
        "serverPorts": [7895, 7895],
        "consolePatchPort": 80,
        "webConsolePort": 8080,
        "checkIntervalMs": 50,
        "restartWaitSeconds": 15.9,
        "autoBroadcastDefault": false,
        "serverExecutable": "   ",
        "shippingExecutable": "  ./custom/Server.exe  ",
        "shippingProcessName": 12345
    });

    let cfg = parse_config_value(&raw);
    let defaults = AppConfig::default();

    assert_eq!(cfg.server_ports, defaults.server_ports);
    assert_eq!(cfg.console_patch_port, defaults.console_patch_port);
    assert_eq!(cfg.web_console_port, 8080);
    assert_eq!(cfg.check_interval_ms, defaults.check_interval_ms);
    assert_eq!(cfg.restart_wait_seconds, 15);
    assert!(!cfg.auto_broadcast_default);
    assert_eq!(cfg.server_executable, defaults.server_executable);
    assert_eq!(cfg.shipping_executable, "./custom/Server.exe");
    assert_eq!(cfg.shipping_process_name, defaults.shipping_process_name);
}

#[test]
fn preserves_valid_distinct_server_ports_and_handles_corrupt_json() {
    let path = temp_config_path();
    fs::write(
        &path,
        r#"{"serverPorts":[9001,9003],"consolePatchPort":9005,"webConsolePort":9007}"#,
    )
    .unwrap();

    let loaded = load_or_create_config_at(&path);
    assert_eq!(loaded.outcome, ConfigLoadOutcome::Loaded);
    assert_eq!(loaded.config.server_ports, [9001, 9003]);
    assert_eq!(loaded.config.console_patch_port, 9005);
    assert_eq!(loaded.config.web_console_port, 9007);

    fs::write(&path, "{ invalid json").unwrap();
    let corrupt = load_or_create_config_at(&path);
    assert!(matches!(corrupt.outcome, ConfigLoadOutcome::LoadFailed(_)));
    assert_eq!(corrupt.config, AppConfig::default());

    let _ = fs::remove_file(&path);
}

#[test]
fn serializes_expected_camel_case_schema() {
    let serialized = serde_json::to_value(AppConfig::default()).unwrap();
    let expected = serde_json::json!({
        "serverPorts": [7895, 7897],
        "consolePatchPort": 7893,
        "webConsolePort": 7891,
        "checkIntervalMs": 1000,
        "restartWaitSeconds": 10,
        "autoBroadcastDefault": true,
        "serverExecutable": "./DreadHungerServer.exe",
        "shippingExecutable": "./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe",
        "shippingProcessName": "DreadHungerServer-Win64-Shipping.exe"
    });
    assert_eq!(serialized, expected);
    assert_eq!(parse_config_value(&serialized), AppConfig::default());
}
