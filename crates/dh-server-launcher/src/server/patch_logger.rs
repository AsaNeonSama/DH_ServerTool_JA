use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

pub use crate::consts::server::{
    MAX_PATCH_RUNTIME_LOG_FILES, PATCH_RUNTIME_LOG_EXTENSION, PATCH_RUNTIME_LOG_PREFIX,
};
use crate::format::{format_log_date_time, format_log_filename_timestamp};
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
}

fn is_valid_timestamp_stem(stem: &str) -> bool {
    let bytes: &[u8] = stem.as_bytes();
    bytes.len() == 15
        && bytes[..8].iter().all(u8::is_ascii_digit)
        && bytes[8] == b'_'
        && bytes[9..15].iter().all(u8::is_ascii_digit)
}

fn parse_patch_log_sort_key(file_name: &str) -> Option<(String, u32)> {
    let stem: &str = file_name
        .strip_prefix(PATCH_RUNTIME_LOG_PREFIX)?
        .strip_suffix(PATCH_RUNTIME_LOG_EXTENSION)?;
    if is_valid_timestamp_stem(stem) {
        return Some((stem.to_string(), 0));
    }
    let (ts_part, seq_part): (&str, &str) = stem.rsplit_once('_')?;
    if !is_valid_timestamp_stem(ts_part)
        || seq_part.is_empty()
        || !seq_part.bytes().all(|b: u8| b.is_ascii_digit())
    {
        return None;
    }
    let seq: u32 = seq_part.parse::<u32>().ok()?;
    (seq >= 1).then(|| (ts_part.to_string(), seq))
}

fn format_patch_log_filename(timestamp_stem: &str, seq: u32) -> String {
    if seq == 0 {
        format!("{PATCH_RUNTIME_LOG_PREFIX}{timestamp_stem}{PATCH_RUNTIME_LOG_EXTENSION}")
    } else {
        format!("{PATCH_RUNTIME_LOG_PREFIX}{timestamp_stem}_{seq:02}{PATCH_RUNTIME_LOG_EXTENSION}")
    }
}

fn next_seq_for_timestamp(dir: &Path, timestamp_stem: &str) -> u32 {
    let Ok(read_dir): Result<fs::ReadDir, std::io::Error> = fs::read_dir(dir) else {
        return 0;
    };
    let mut max_seq: Option<u32> = None;

    for entry in read_dir.flatten() {
        let is_file: bool = entry.file_type().is_ok_and(|ft: fs::FileType| ft.is_file());
        if !is_file {
            continue;
        }
        let file_name_os: OsString = entry.file_name();
        let Some(name_str): Option<&str> = file_name_os.to_str() else {
            continue;
        };
        if let Some((ts, seq)) = parse_patch_log_sort_key(name_str) {
            if ts == timestamp_stem {
                max_seq = Some(max_seq.map_or(seq, |prev: u32| prev.max(seq)));
            }
        }
    }

    max_seq.map_or(0, |prev: u32| prev.saturating_add(1))
}

fn create_unique_log_file(dir: &Path, timestamp_stem: &str, header: &[u8]) -> PathBuf {
    let start_seq: u32 = next_seq_for_timestamp(dir, timestamp_stem);
    let end_seq: u32 = start_seq.saturating_add(9_999);

    for seq in start_seq..=end_seq {
        let candidate_name: String = format_patch_log_filename(timestamp_stem, seq);
        let candidate: PathBuf = dir.join(candidate_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                let _ = file.write_all(header);
                return candidate;
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
            Err(_) => {
                let _ = fs::write(&candidate, header);
                return candidate;
            }
        }
    }

    let fallback_path: PathBuf = dir.join(format_patch_log_filename(timestamp_stem, start_seq));
    let _ = fs::write(&fallback_path, header);
    fallback_path
}

fn prune_old_patch_runtime_logs(dir: &Path, keep_path: &Path) {
    let Ok(read_dir): Result<fs::ReadDir, std::io::Error> = fs::read_dir(dir) else {
        return;
    };
    let keep_file_name: Option<&OsStr> = keep_path.file_name();
    let mut others: Vec<(String, u32, PathBuf)> = Vec::new();

    for entry in read_dir.flatten() {
        let is_file: bool = entry.file_type().is_ok_and(|ft: fs::FileType| ft.is_file());
        if !is_file {
            continue;
        }
        let file_name_os: OsString = entry.file_name();
        if keep_file_name.is_some_and(|keep: &OsStr| keep == file_name_os.as_os_str()) {
            continue;
        }
        let Some(name_str): Option<&str> = file_name_os.to_str() else {
            continue;
        };
        let Some((ts, seq)): Option<(String, u32)> = parse_patch_log_sort_key(name_str) else {
            continue;
        };
        others.push((ts, seq, entry.path()));
    }

    let max_others: usize = MAX_PATCH_RUNTIME_LOG_FILES.saturating_sub(1);
    if others.len() <= max_others {
        return;
    }

    others.sort_by(|a: &(String, u32, PathBuf), b: &(String, u32, PathBuf)| {
        a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1))
    });
    let remove_count: usize = others.len() - max_others;
    for (_, _, old_path) in others.into_iter().take(remove_count) {
        let _ = fs::remove_file(old_path);
    }
}

/// Creates a new launch-timestamped `./patch_runtime_YYYYMMDD_HHMMSS.log` in the current
/// launcher/server root directory and prunes older launch logs so only the newest
/// `MAX_PATCH_RUNTIME_LOG_FILES` matches remain.
#[must_use]
pub fn start_patch_runtime_log(port: u16, patches: &[ResolvedPatchScript]) -> PathBuf {
    start_patch_runtime_log_in(Path::new("."), port, patches)
}

/// Creates a new launch-timestamped `patch_runtime_YYYYMMDD_HHMMSS.log` inside `dir`,
/// writes the match header, and prunes older matching log files to keep only the newest
/// `MAX_PATCH_RUNTIME_LOG_FILES` matches in chronological order.
#[must_use]
pub fn start_patch_runtime_log_in(
    dir: &Path,
    port: u16,
    patches: &[ResolvedPatchScript],
) -> PathBuf {
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
    let file_timestamp: String = format_log_filename_timestamp();
    let log_path: PathBuf = create_unique_log_file(dir, &file_timestamp, header.as_bytes());
    prune_old_patch_runtime_logs(dir, &log_path);
    log_path
}

/// Appends one formatted UTF-8 runtime entry to `path`.
/// Never prints runtime JS logs to Tab 2.
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
