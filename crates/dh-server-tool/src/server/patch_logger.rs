use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

pub use crate::consts::server::PATCH_RUNTIME_LOG_RELATIVE_PATH;
use crate::format::format_log_date_time;
use crate::languages;
use crate::mods::ResolvedPatchScript;
use crate::structs::languages::ServerMsg;
pub use crate::structs::server::PatchLogLevel;

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
    let timestamp: String = format_log_date_time();
    let header: String = languages::render_server(&ServerMsg::PatchLogHeader {
        timestamp: &timestamp,
        port,
        patch_names: &patch_names,
    });
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
