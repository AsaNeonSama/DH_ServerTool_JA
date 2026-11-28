//! Opt-in relative diagnostic logger for `dh-server-voice`.
//!
//! Disabled by default (zero I/O overhead). Set `DH_VOICE_SERVER_LOG=1` to write
//! diagnostic lines to `./dh_voice_server.log` in the current working directory.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

use crate::consts::patch::{ENV_ENABLE_LOG, RELATIVE_LOG_FILE};

static LOG_ENABLED: OnceLock<bool> = OnceLock::new();
static LOG_FILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();

/// Returns whether opt-in diagnostic file logging is enabled.
#[must_use]
pub fn is_logging_enabled() -> bool {
    *LOG_ENABLED.get_or_init(|| {
        std::env::var(ENV_ENABLE_LOG)
            .is_ok_and(|val: String| val == "1" || val.eq_ignore_ascii_case("true"))
    })
}

/// Appends a single diagnostic line to `./dh_voice_server.log` when logging is enabled.
pub fn log_line(msg: &str) {
    if !is_logging_enabled() {
        return;
    }
    let mutex: &Mutex<Option<File>> = LOG_FILE.get_or_init(|| {
        let opened: Option<File> = OpenOptions::new()
            .create(true)
            .append(true)
            .open(RELATIVE_LOG_FILE)
            .ok();
        Mutex::new(opened)
    });
    if let Ok(mut guard) = mutex.lock() {
        if let Some(file) = guard.as_mut() {
            let _res: std::io::Result<()> = writeln!(file, "[dh_voice_server] {msg}");
            let _flush: std::io::Result<()> = file.flush();
        }
    }
}
