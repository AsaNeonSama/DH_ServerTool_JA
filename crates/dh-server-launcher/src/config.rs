use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

pub use crate::consts::config::{
    CONFIG_RELATIVE_PATH, MAX_VALID_PORT, MIN_RESTART_WAIT_SECONDS, MIN_VALID_PORT,
};
use crate::consts::config::{
    DEFAULT_AUTO_BROADCAST, DEFAULT_CONSOLE_PATCH_PORT, DEFAULT_RESTART_WAIT_SECONDS,
    DEFAULT_SERVER_EXECUTABLE, DEFAULT_SERVER_PORTS, DEFAULT_SHIPPING_EXECUTABLE,
    DEFAULT_SHIPPING_PROCESS_NAME, DEFAULT_SWITCH_PORT_ON_RESTART, MAX_CONFIG_FILE_BYTES,
    MAX_SAFE_INT_F64,
};
pub use crate::structs::config::{AppConfig, ConfigLoadOutcome, LoadedConfig};

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_ports: DEFAULT_SERVER_PORTS,
            switch_port_on_restart: DEFAULT_SWITCH_PORT_ON_RESTART,
            console_patch_port: DEFAULT_CONSOLE_PATCH_PORT,
            restart_wait_seconds: DEFAULT_RESTART_WAIT_SECONDS,
            auto_broadcast_default: DEFAULT_AUTO_BROADCAST,
            server_executable: DEFAULT_SERVER_EXECUTABLE.to_string(),
            shipping_executable: DEFAULT_SHIPPING_EXECUTABLE.to_string(),
            shipping_process_name: DEFAULT_SHIPPING_PROCESS_NAME.to_string(),
        }
    }
}

fn fallback_second_port(first_port: u16, defaults: [u16; 2]) -> u16 {
    if first_port == defaults[1] {
        defaults[0]
    } else {
        defaults[1]
    }
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
/// and cleanly ignoring legacy fields (`webConsolePort`, `webConsolePsk`, `checkIntervalMs`, `pollIntervalMs`).
#[must_use]
pub fn parse_config_value(raw: &Value) -> AppConfig {
    let defaults: AppConfig = AppConfig::default();
    let Some(obj): Option<&Map<String, Value>> = raw.as_object() else {
        return defaults;
    };

    let switch_port_on_restart: bool = obj
        .get("switchPortOnRestart")
        .and_then(Value::as_bool)
        .unwrap_or(defaults.switch_port_on_restart);

    let server_ports: [u16; 2] = match obj.get("serverPorts").and_then(Value::as_array) {
        Some(arr) if arr.len() >= 2 => {
            match (parse_valid_port(arr.first()), parse_valid_port(arr.get(1))) {
                (Some(p0), Some(p1)) if p0 != p1 => [p0, p1],
                (Some(p0), _) if !switch_port_on_restart => {
                    [p0, fallback_second_port(p0, defaults.server_ports)]
                }
                _ => defaults.server_ports,
            }
        }
        Some(arr) if !switch_port_on_restart && arr.len() == 1 => parse_valid_port(arr.first())
            .map_or(defaults.server_ports, |p0: u16| {
                [p0, fallback_second_port(p0, defaults.server_ports)]
            }),
        _ => defaults.server_ports,
    };

    let console_patch_port: u16 =
        parse_valid_port(obj.get("consolePatchPort")).unwrap_or(defaults.console_patch_port);
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
        switch_port_on_restart,
        console_patch_port,
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

/// Loads `launcher_config.json` from `path`, creating it with defaults if missing.
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

    let parsed_result: Result<Value, String> = fs::metadata(path)
        .map_err(|e: std::io::Error| e.to_string())
        .and_then(|meta: fs::Metadata| {
            if meta.len() > MAX_CONFIG_FILE_BYTES {
                return Err(format!(
                    "config file exceeds maximum size ({} > {MAX_CONFIG_FILE_BYTES} bytes)",
                    meta.len()
                ));
            }
            fs::read_to_string(path).map_err(|e: std::io::Error| e.to_string())
        })
        .and_then(|content: String| {
            let trimmed_bom: &str = content.strip_prefix('\u{FEFF}').unwrap_or(&content);
            serde_json::from_str::<Value>(trimmed_bom).map_err(|e: serde_json::Error| e.to_string())
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

/// Loads or creates `./launcher_config.json` in the current working directory.
#[must_use]
pub fn load_or_create_config() -> LoadedConfig {
    load_or_create_config_at(Path::new(CONFIG_RELATIVE_PATH))
}

/// Validates an `AppConfig` before persisting it from the desktop GUI.
pub fn validate_app_config(config: &AppConfig) -> Result<(), String> {
    if !is_valid_port_number(config.console_patch_port) {
        return Err(format!(
            "内部パッチポートは {MIN_VALID_PORT}〜{MAX_VALID_PORT} の範囲で指定してください。"
        ));
    }
    if !is_valid_port_number(config.server_ports[0]) {
        return Err(format!(
            "ゲームサーバーポート1は {MIN_VALID_PORT}〜{MAX_VALID_PORT} の範囲で指定してください。"
        ));
    }
    if config.switch_port_on_restart {
        if !is_valid_port_number(config.server_ports[1]) {
            return Err(format!(
                "ゲームサーバーポート2は {MIN_VALID_PORT}〜{MAX_VALID_PORT} の範囲で指定してください。"
            ));
        }
        if config.server_ports[0] == config.server_ports[1] {
            return Err(
                "ポート切替ON時は、ゲームサーバーポート1とポート2に別の番号を指定してください。"
                    .to_string(),
            );
        }
    }
    let check_ports: &[u16] = if config.switch_port_on_restart {
        &config.server_ports[..]
    } else {
        &config.server_ports[..1]
    };
    for &srv_port in check_ports {
        if srv_port == config.console_patch_port {
            return Err(format!(
                "ゲームサーバーポート ({srv_port}) が内部コンソール連携ポート ({}) と重複しています。",
                config.console_patch_port
            ));
        }
    }
    if config.restart_wait_seconds < MIN_RESTART_WAIT_SECONDS {
        return Err(format!(
            "再起動カウントダウンは {MIN_RESTART_WAIT_SECONDS}秒 以上を指定してください。"
        ));
    }
    if config.shipping_executable.trim().is_empty() {
        return Err("サーバー実行ファイルのパスを入力してください。".to_string());
    }
    if config.shipping_process_name.trim().is_empty() {
        return Err("サーバープロセス名を入力してください。".to_string());
    }
    Ok(())
}

/// Validates and persists `config` to `path` using the canonical camelCase JSON schema.
pub fn save_config_at(path: &Path, config: &AppConfig) -> Result<AppConfig, String> {
    let mut normalized: AppConfig = config.clone();
    normalized.server_executable = normalized.server_executable.trim().to_string();
    normalized.shipping_executable = normalized.shipping_executable.trim().to_string();
    normalized.shipping_process_name = normalized.shipping_process_name.trim().to_string();
    if normalized.server_executable.is_empty() {
        normalized.server_executable = DEFAULT_SERVER_EXECUTABLE.to_string();
    }
    if !normalized.switch_port_on_restart
        && normalized.server_ports[0] == normalized.server_ports[1]
    {
        normalized.server_ports[1] =
            fallback_second_port(normalized.server_ports[0], DEFAULT_SERVER_PORTS);
    }

    validate_app_config(&normalized)?;

    let mut json_text: String =
        serde_json::to_string_pretty(&normalized).map_err(|e: serde_json::Error| {
            format!("launcher_config.json のシリアライズに失敗しました: {e}")
        })?;
    json_text.push('\n');
    fs::write(path, json_text.as_bytes()).map_err(|e: std::io::Error| {
        format!("launcher_config.json の書き込みに失敗しました: {e}")
    })?;
    Ok(normalized)
}

/// Validates and persists `./launcher_config.json` in the current working directory.
pub fn save_config(config: &AppConfig) -> Result<AppConfig, String> {
    save_config_at(Path::new(CONFIG_RELATIVE_PATH), config)
}
