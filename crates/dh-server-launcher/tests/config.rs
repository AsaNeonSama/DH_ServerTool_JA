use std::env;
use std::fs;
use std::path::PathBuf;

use dh_server_launcher::config::{
    load_or_create_config_at, parse_config_value, AppConfig, ConfigLoadOutcome,
    CONFIG_RELATIVE_PATH,
};
use serde_json::Value;
use uuid::Uuid;

fn temp_config_path() -> PathBuf {
    env::temp_dir().join(format!("dh_config_test_{}.json", Uuid::new_v4()))
}

#[test]
fn creates_default_config_when_missing() {
    assert_eq!(CONFIG_RELATIVE_PATH, "./launcher_config.json");
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
fn falls_back_field_by_field_for_invalid_values_and_ignores_legacy_keys() {
    let raw: Value = serde_json::json!({
        "serverPorts": [7895, 7895],
        "consolePatchPort": 80,
        "webConsolePort": 8080,
        "webConsolePsk": "legacy-ignored-key",
        "checkIntervalMs": 50,
        "pollIntervalMs": 250,
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
    assert_eq!(cfg.restart_wait_seconds, 15);
    assert!(!cfg.auto_broadcast_default);
    assert_eq!(cfg.server_executable, defaults.server_executable);
    assert_eq!(cfg.shipping_executable, "./custom/Server.exe");
    assert_eq!(cfg.shipping_process_name, defaults.shipping_process_name);

    // Direct serde deserialization must also cleanly ignore legacy checkIntervalMs / pollIntervalMs
    let legacy_serde: AppConfig = serde_json::from_value(serde_json::json!({
        "serverPorts": [7895, 7897],
        "switchPortOnRestart": true,
        "consolePatchPort": 7893,
        "checkIntervalMs": 1000,
        "pollIntervalMs": 500,
        "webConsolePort": 7891,
        "restartWaitSeconds": 10,
        "autoBroadcastDefault": true,
        "serverExecutable": "./DreadHungerServer.exe",
        "shippingExecutable": "./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe",
        "shippingProcessName": "DreadHungerServer-Win64-Shipping.exe"
    }))
    .expect("legacy config with checkIntervalMs/pollIntervalMs must deserialize");
    assert_eq!(legacy_serde, defaults);
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
    assert!(loaded.config.switch_port_on_restart);
    assert_eq!(loaded.config.console_patch_port, 9005);

    fs::write(&path, "{ invalid json").unwrap();
    let corrupt = load_or_create_config_at(&path);
    assert!(matches!(corrupt.outcome, ConfigLoadOutcome::LoadFailed(_)));
    assert_eq!(corrupt.config, AppConfig::default());

    // UTF-8 BOM-prefixed launcher_config.json (e.g. saved from Windows Notepad) must parse cleanly.
    fs::write(
        &path,
        "\u{FEFF}{\"serverPorts\":[9101,9103],\"switchPortOnRestart\":false}",
    )
    .unwrap();
    let bom_loaded = load_or_create_config_at(&path);
    assert_eq!(bom_loaded.outcome, ConfigLoadOutcome::Loaded);
    assert_eq!(bom_loaded.config.server_ports, [9101, 9103]);
    assert!(!bom_loaded.config.switch_port_on_restart);

    let _ = fs::remove_file(&path);
}

#[test]
fn supports_switch_port_on_restart_and_legacy_defaults() {
    // Legacy config without switchPortOnRestart must default to true.
    let legacy = serde_json::json!({
        "serverPorts": [7895, 7897]
    });
    let parsed_legacy = parse_config_value(&legacy);
    assert!(parsed_legacy.switch_port_on_restart);
    assert_eq!(parsed_legacy.server_ports, [7895, 7897]);

    // Invalid non-boolean switchPortOnRestart falls back to default true.
    let invalid_type = serde_json::json!({
        "switchPortOnRestart": "no",
        "serverPorts": [9001, 9001]
    });
    let parsed_invalid = parse_config_value(&invalid_type);
    assert!(parsed_invalid.switch_port_on_restart);
    assert_eq!(
        parsed_invalid.server_ports,
        AppConfig::default().server_ports
    );

    // Fixed-port mode (switchPortOnRestart: false) accepts single-port or duplicate pair entries.
    let fixed_single = serde_json::json!({
        "switchPortOnRestart": false,
        "serverPorts": [9001]
    });
    let parsed_fixed_single = parse_config_value(&fixed_single);
    assert!(!parsed_fixed_single.switch_port_on_restart);
    assert_eq!(parsed_fixed_single.server_ports[0], 9001);
    assert_ne!(
        parsed_fixed_single.server_ports[0],
        parsed_fixed_single.server_ports[1]
    );

    let fixed_dup = serde_json::json!({
        "switchPortOnRestart": false,
        "serverPorts": [7897, 7897]
    });
    let parsed_fixed_dup = parse_config_value(&fixed_dup);
    assert!(!parsed_fixed_dup.switch_port_on_restart);
    assert_eq!(parsed_fixed_dup.server_ports, [7897, 7895]);
}

#[test]
fn serializes_expected_eight_field_camel_case_schema_without_legacy_fields() {
    let serialized = serde_json::to_value(AppConfig::default()).unwrap();
    let expected = serde_json::json!({
        "serverPorts": [7895, 7897],
        "switchPortOnRestart": true,
        "consolePatchPort": 7893,
        "restartWaitSeconds": 10,
        "autoBroadcastDefault": true,
        "serverExecutable": "./DreadHungerServer.exe",
        "shippingExecutable": "./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe",
        "shippingProcessName": "DreadHungerServer-Win64-Shipping.exe"
    });
    assert_eq!(serialized, expected);
    assert!(serialized.get("webConsolePort").is_none());
    assert!(serialized.get("checkIntervalMs").is_none());
    assert!(serialized.get("pollIntervalMs").is_none());
    assert_eq!(parse_config_value(&serialized), AppConfig::default());
}
