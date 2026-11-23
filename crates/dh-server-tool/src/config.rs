use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const CONFIG_RELATIVE_PATH: &str = "./config.json";
pub const MIN_VALID_PORT: u16 = 1024;
pub const MAX_VALID_PORT: u16 = 65535;
pub const MIN_CHECK_INTERVAL_MS: u64 = 200;
pub const MIN_RESTART_WAIT_SECONDS: u64 = 1;

const MAX_SAFE_INT_F64: f64 = 9_007_199_254_740_991.0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub server_ports: [u16; 2],
    pub console_patch_port: u16,
    pub web_console_port: u16,
    pub check_interval_ms: u64,
    pub restart_wait_seconds: u64,
    pub auto_broadcast_default: bool,
    pub server_executable: String,
    pub shipping_executable: String,
    pub shipping_process_name: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_ports: [7895, 7897],
            console_patch_port: 7893,
            web_console_port: 7891,
            check_interval_ms: 1000,
            restart_wait_seconds: 10,
            auto_broadcast_default: true,
            server_executable: "./DreadHungerServer.exe".to_string(),
            shipping_executable:
                "./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe".to_string(),
            shipping_process_name: "DreadHungerServer-Win64-Shipping.exe".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLoadOutcome {
    Loaded,
    CreatedDefault,
    CreateFailed,
    LoadFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub config: AppConfig,
    pub outcome: ConfigLoadOutcome,
}

#[must_use]
pub fn is_valid_port_number(port: u16) -> bool {
    port >= MIN_VALID_PORT
}

fn f64_floor_to_u64(value: f64) -> Option<u64> {
    if !value.is_finite() || value < 0.0 || value > MAX_SAFE_INT_F64 {
        return None;
    }
    let floored_text: String = format!("{:.0}", value.floor());
    floored_text.parse::<u64>().ok()
}

fn parse_valid_port(value: Option<&Value>) -> Option<u16> {
    let val: &Value = value?;
    if let Some(n) = val.as_u64() {
        if n >= u64::from(MIN_VALID_PORT) && n <= u64::from(MAX_VALID_PORT) {
            return u16::try_from(n).ok();
        }
        return None;
    }
    if let Some(f) = val.as_f64() {
        if f.is_finite()
            && f.fract() == 0.0
            && f >= f64::from(MIN_VALID_PORT)
            && f <= f64::from(MAX_VALID_PORT)
        {
            let as_u64: u64 = f64_floor_to_u64(f)?;
            return u16::try_from(as_u64).ok();
        }
    }
    None
}

fn read_non_empty_string(value: Option<&Value>, fallback: &str) -> String {
    match value.and_then(Value::as_str) {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => fallback.to_string(),
    }
}

fn read_min_int(value: Option<&Value>, min_value: u64, fallback: u64) -> u64 {
    let Some(val): Option<&Value> = value else {
        return fallback;
    };
    if let Some(n) = val.as_u64() {
        if n >= min_value {
            return n;
        }
        return fallback;
    }
    if let Some(f) = val.as_f64() {
        if let Some(n) = f64_floor_to_u64(f) {
            if n >= min_value {
                return n;
            }
        }
    }
    fallback
}

/// Parses a JSON value into an `AppConfig`, falling back field-by-field to defaults
/// for missing or invalid entries (1:1 parity with `config.ts`).
#[must_use]
pub fn parse_config_value(raw: &Value) -> AppConfig {
    let defaults: AppConfig = AppConfig::default();
    let Some(obj): Option<&Map<String, Value>> = raw.as_object() else {
        return defaults;
    };

    let server_ports: [u16; 2] = match obj.get("serverPorts").and_then(Value::as_array) {
        Some(arr) if arr.len() >= 2 => {
            match (parse_valid_port(arr.first()), parse_valid_port(arr.get(1))) {
                (Some(p0), Some(p1)) if p0 != p1 => [p0, p1],
                _ => defaults.server_ports,
            }
        }
        _ => defaults.server_ports,
    };

    let console_patch_port: u16 =
        parse_valid_port(obj.get("consolePatchPort")).unwrap_or(defaults.console_patch_port);
    let web_console_port: u16 =
        parse_valid_port(obj.get("webConsolePort")).unwrap_or(defaults.web_console_port);
    let check_interval_ms: u64 = read_min_int(
        obj.get("checkIntervalMs"),
        MIN_CHECK_INTERVAL_MS,
        defaults.check_interval_ms,
    );
    let restart_wait_seconds: u64 = read_min_int(
        obj.get("restartWaitSeconds"),
        MIN_RESTART_WAIT_SECONDS,
        defaults.restart_wait_seconds,
    );
    let auto_broadcast_default: bool = obj
        .get("autoBroadcastDefault")
        .and_then(Value::as_bool)
        .unwrap_or(defaults.auto_broadcast_default);

    AppConfig {
        server_ports,
        console_patch_port,
        web_console_port,
        check_interval_ms,
        restart_wait_seconds,
        auto_broadcast_default,
        server_executable: read_non_empty_string(
            obj.get("serverExecutable"),
            &defaults.server_executable,
        ),
        shipping_executable: read_non_empty_string(
            obj.get("shippingExecutable"),
            &defaults.shipping_executable,
        ),
        shipping_process_name: read_non_empty_string(
            obj.get("shippingProcessName"),
            &defaults.shipping_process_name,
        ),
    }
}

/// Ensures the current working directory contains `server_executable` using relative paths only.
/// If not found in the current working directory, checks the directory of `current_exe()`
/// and switches cwd there when present.
#[must_use]
pub fn ensure_server_working_dir(server_executable: &str) -> bool {
    let rel: &Path = Path::new(server_executable);
    if rel.exists() {
        return true;
    }
    let Ok(exe_path): Result<PathBuf, std::io::Error> = env::current_exe() else {
        return false;
    };
    let Some(exe_dir): Option<&Path> = exe_path.parent() else {
        return false;
    };
    if exe_dir.join(rel).exists() && env::set_current_dir(exe_dir).is_ok() {
        return rel.exists();
    }
    false
}

/// Loads `config.json` from `path`, creating it with defaults if missing.
#[must_use]
pub fn load_or_create_config_at(path: &Path) -> LoadedConfig {
    let defaults: AppConfig = AppConfig::default();
    if !path.exists() {
        let formatted: String =
            serde_json::to_string_pretty(&defaults).map_or_default(|mut s: String| {
                s.push('\n');
                s
            });
        let outcome: ConfigLoadOutcome = match fs::write(path, formatted) {
            Ok(()) => ConfigLoadOutcome::CreatedDefault,
            Err(_) => ConfigLoadOutcome::CreateFailed,
        };
        return LoadedConfig {
            config: defaults,
            outcome,
        };
    }

    let parsed_result: Result<Value, String> = fs::read_to_string(path)
        .map_err(|e: std::io::Error| e.to_string())
        .and_then(|content: String| {
            serde_json::from_str::<Value>(&content).map_err(|e: serde_json::Error| e.to_string())
        });

    match parsed_result {
        Ok(raw) => LoadedConfig {
            config: parse_config_value(&raw),
            outcome: ConfigLoadOutcome::Loaded,
        },
        Err(err) => LoadedConfig {
            config: defaults,
            outcome: ConfigLoadOutcome::LoadFailed(err),
        },
    }
}

/// Loads or creates `./config.json` in the current working directory.
#[must_use]
pub fn load_or_create_config() -> LoadedConfig {
    load_or_create_config_at(Path::new(CONFIG_RELATIVE_PATH))
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
