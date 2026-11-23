use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::format::format_log_date_time;
use crate::messages::ja::server as server_msg;
use crate::mods::ResolvedPatchScript;

pub const PATCH_RUNTIME_LOG_RELATIVE_PATH: &str = "./patch_runtime.log";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchLogLevel {
    Info,
    Warn,
    Error,
}

impl PatchLogLevel {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }

    #[must_use]
    pub fn from_wire_level(level: &str) -> Self {
        if level.eq_ignore_ascii_case("error") {
            Self::Error
        } else if level.eq_ignore_ascii_case("warn") || level.eq_ignore_ascii_case("warning") {
            Self::Warn
        } else {
            Self::Info
        }
    }
}

/// Overwrites `./patch_runtime.log` in the current launcher/server root directory
/// at the start of every match round so only the latest match's runtime logs are kept.
pub fn reset_patch_runtime_log(port: u16, patches: &[ResolvedPatchScript]) {
    reset_patch_runtime_log_at(Path::new(PATCH_RUNTIME_LOG_RELATIVE_PATH), port, patches);
}

pub fn reset_patch_runtime_log_at(path: &Path, port: u16, patches: &[ResolvedPatchScript]) {
    let patch_names: String = patches
        .iter()
        .map(|p: &ResolvedPatchScript| p.file_name.as_str())
        .collect::<Vec<&str>>()
        .join(", ");
    let header: String = server_msg::patch_log_header(&format_log_date_time(), port, &patch_names);
    let _ = fs::write(path, header.as_bytes());
}

/// Appends one formatted UTF-8 runtime entry to `./patch_runtime.log`.
/// Never prints runtime JS logs to Tab 2.
pub fn append_patch_runtime_log(level: PatchLogLevel, file_name: &str, message: &str) {
    append_patch_runtime_log_at(
        Path::new(PATCH_RUNTIME_LOG_RELATIVE_PATH),
        level,
        file_name,
        message,
    );
}

pub fn append_patch_runtime_log_at(
    path: &Path,
    level: PatchLogLevel,
    file_name: &str,
    message: &str,
) {
    let line: String = format!(
        "[{}] [{}] [{file_name}] {message}\n",
        format_log_date_time(),
        level.as_str(),
    );
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::build_injection_plan;
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
}
