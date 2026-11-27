use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::consts::patch::{VOICE_LOG_ENV_VAR, VOICE_LOG_FILE_NAME};

static LOG_ENABLED: OnceLock<bool> = OnceLock::new();
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_MUTEX: Mutex<()> = Mutex::new(());

/// Returns whether opt-in runtime diagnostic logging (`DH_VOICE_LOG=1`) is enabled.
#[must_use]
pub fn is_logging_enabled() -> bool {
    *LOG_ENABLED.get_or_init(|| {
        let raw_opt: Option<String> = std::env::var(VOICE_LOG_ENV_VAR).ok();
        raw_opt.is_some_and(|val: String| {
            let trimmed: &str = val.trim();
            trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
        })
    })
}

fn resolved_log_path() -> &'static PathBuf {
    LOG_PATH.get_or_init(|| {
        std::env::current_dir().map_or_else(
            |_err: std::io::Error| PathBuf::from(VOICE_LOG_FILE_NAME),
            |cwd: PathBuf| cwd.join(VOICE_LOG_FILE_NAME),
        )
    })
}

/// Appends a timestamped diagnostic line to `<initial_cwd>/dh_voice_client.log` when `DH_VOICE_LOG=1`.
pub fn log_line(message: &str) {
    if !is_logging_enabled() {
        return;
    }

    let Ok(_guard): Result<std::sync::MutexGuard<'_, ()>, _> = LOG_MUTEX.lock() else {
        return;
    };

    let millis: u128 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |dur: std::time::Duration| dur.as_millis());

    let log_path: &PathBuf = resolved_log_path();
    let Ok(mut file): Result<std::fs::File, std::io::Error> =
        OpenOptions::new().create(true).append(true).open(log_path)
    else {
        return;
    };

    let _written: Result<(), std::io::Error> =
        writeln!(file, "[{millis}] [dh-client-voice] {message}");
}
