use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::consts::config::{CONFIG_FILE_NAME, DEFAULT_EXE_RELATIVE_PATH};
use crate::consts::messages::MODE_PLAIN;
use crate::structs::builtin_dlls::BuiltinDllConfig;
use crate::structs::config::{FileStatuses, LauncherConfig};

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            exe_path: DEFAULT_EXE_RELATIVE_PATH.to_string(),
            builtin: BuiltinDllConfig::default(),
        }
    }
}

/// Resolves the launcher base directory (current working directory, falling back to `.`).
#[must_use]
pub fn resolve_base_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_: std::io::Error| PathBuf::from("."))
}

fn clean_path_components(path: &Path) -> PathBuf {
    let mut cleaned: PathBuf = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop: bool =
                    matches!(cleaned.components().next_back(), Some(Component::Normal(_)));
                if can_pop {
                    let _ = cleaned.pop();
                } else {
                    cleaned.push(component.as_os_str());
                }
            }
            other => cleaned.push(other.as_os_str()),
        }
    }
    if cleaned.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        cleaned
    }
}

/// Resolves a configured relative or absolute path against `base_dir`,
/// stripping redundant `.` segments and normalizing path separators.
#[must_use]
pub fn resolve_entry_path(base_dir: &Path, raw: &str) -> PathBuf {
    let trimmed: &str = raw.trim();
    let candidate: PathBuf = PathBuf::from(trimmed);
    let combined: PathBuf = if candidate.is_absolute() {
        candidate
    } else {
        base_dir.join(candidate)
    };
    clean_path_components(&combined)
}

/// Formats `path` relative to `base_dir` (`./...` with forward slashes) when inside `base_dir`,
/// keeping external selections portable without hardcoding workspace paths.
#[must_use]
pub fn normalize_display_path(base_dir: &Path, path: &Path) -> String {
    let canonical_base: PathBuf =
        fs::canonicalize(base_dir).unwrap_or_else(|_: std::io::Error| base_dir.to_path_buf());
    let canonical_path: PathBuf =
        fs::canonicalize(path).unwrap_or_else(|_: std::io::Error| path.to_path_buf());

    if let Ok(rel) = canonical_path.strip_prefix(&canonical_base) {
        let rel_str: String = rel.to_string_lossy().replace('\\', "/");
        if rel_str.is_empty() {
            return ".".to_string();
        }
        return format!("./{rel_str}");
    }

    if let Ok(rel) = path.strip_prefix(base_dir) {
        let rel_str: String = rel.to_string_lossy().replace('\\', "/");
        if !rel_str.is_empty() {
            return format!("./{rel_str}");
        }
    }

    path.to_string_lossy().to_string()
}

/// Loads `./launcher_config.json` from `base_dir`, or creates it with relative defaults
/// and built-in client MOD toggles enabled on first run.
pub fn load_or_create_config_in(base_dir: &Path) -> Result<LauncherConfig, String> {
    let config_path: PathBuf = base_dir.join(CONFIG_FILE_NAME);
    if config_path.is_file() {
        let raw: String = fs::read_to_string(&config_path)
            .map_err(|err: std::io::Error| format!("設定ファイルの読込に失敗しました: {err}"))?;
        let mut parsed: LauncherConfig =
            serde_json::from_str(&raw).map_err(|err: serde_json::Error| {
                format!("設定ファイルのJSON解析に失敗しました: {err}")
            })?;
        if parsed.exe_path.trim().is_empty() {
            parsed.exe_path = DEFAULT_EXE_RELATIVE_PATH.to_string();
        }
        return Ok(parsed);
    }

    let config: LauncherConfig = LauncherConfig::default();
    save_config_in(base_dir, &config)?;
    Ok(config)
}

/// Persists `config` to `<base_dir>/launcher_config.json` with pretty UTF-8 JSON.
pub fn save_config_in(base_dir: &Path, config: &LauncherConfig) -> Result<(), String> {
    let config_path: PathBuf = base_dir.join(CONFIG_FILE_NAME);
    let json: String = serde_json::to_string_pretty(config)
        .map_err(|err: serde_json::Error| format!("設定JSONの生成に失敗しました: {err}"))?;
    fs::write(&config_path, format!("{json}\n"))
        .map_err(|err: std::io::Error| format!("設定ファイルの保存に失敗しました: {err}"))?;
    Ok(())
}

/// Evaluates whether the configured target executable currently exists on disk.
#[must_use]
pub fn evaluate_file_statuses(base_dir: &Path, config: &LauncherConfig) -> FileStatuses {
    let exe_trimmed: &str = config.exe_path.trim();
    let exe_exists: bool =
        !exe_trimmed.is_empty() && resolve_entry_path(base_dir, exe_trimmed).is_file();
    FileStatuses { exe_exists }
}

/// Formats a human-readable Japanese launch mode summary for active built-in DLL
/// and built-in Frida JS counts.
#[must_use]
pub fn describe_launch_mode(builtin_dlls: usize, builtin_scripts: usize) -> String {
    match (builtin_dlls, builtin_scripts) {
        (0, 0) => MODE_PLAIN.to_string(),
        (d, 0) => format!("内蔵DLL ({d}件 / Frida未使用)"),
        (0, s) => format!("内蔵JS ({s}件)"),
        (d, s) => format!("内蔵DLL ({d}件) + 内蔵JS ({s}件)"),
    }
}
