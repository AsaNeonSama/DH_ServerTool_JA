use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{normalize_display_path, resolve_entry_path};
use crate::consts::discovery::{
    RELATIVE_EXE_CANDIDATES, STEAM_COMMON_SUFFIX, STEAM_LIBRARY_VDF_RELATIVE_PATHS,
    STEAM_ROOT_CANDIDATES,
};
use crate::structs::discovery::AutoDetectResponse;

/// Parses Steam `libraryfolders.vdf` contents and extracts library root paths.
#[must_use]
pub fn parse_steam_library_folders_vdf(content: &str) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for line in content.lines() {
        let tokens: Vec<String> = extract_quoted_vdf_tokens(line);
        if tokens.len() >= 2 && tokens[0].eq_ignore_ascii_case("path") {
            let unescaped: String = tokens[1].replace("\\\\", "\\");
            let trimmed: &str = unescaped.trim();
            if trimmed.is_empty() {
                continue;
            }
            let dedup: String = trimmed.to_ascii_lowercase();
            if seen.insert(dedup) {
                roots.push(PathBuf::from(trimmed));
            }
        }
    }

    roots
}

fn extract_quoted_vdf_tokens(line: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current: String = String::new();
    let mut in_quotes: bool = false;

    for ch in line.chars() {
        if ch == '"' {
            if in_quotes {
                tokens.push(std::mem::take(&mut current));
                in_quotes = false;
            } else {
                in_quotes = true;
            }
        } else if in_quotes {
            current.push(ch);
        }
    }

    tokens
}

/// Attempts to locate `DreadHunger-Win64-Shipping.exe` by checking relative paths first,
/// followed by standard Steam library paths and parsed `libraryfolders.vdf` entries across drives.
#[must_use]
pub fn auto_detect_client_exe(base_dir: &Path, include_steam_drives: bool) -> AutoDetectResponse {
    let mut checked: usize = 0;

    for rel in RELATIVE_EXE_CANDIDATES {
        checked = checked.saturating_add(1);
        let candidate: PathBuf = resolve_entry_path(base_dir, rel);
        if candidate.is_file() {
            return AutoDetectResponse {
                found: Some((*rel).to_string()),
                checked_count: checked,
            };
        }
    }

    if !include_steam_drives {
        return AutoDetectResponse {
            found: None,
            checked_count: checked,
        };
    }

    let mut steam_roots: Vec<PathBuf> = Vec::new();
    let mut seen_roots: HashSet<String> = HashSet::new();

    for drive_byte in b'C'..=b'Z' {
        let drive_letter: char = char::from(drive_byte);
        let drive_root: String = format!("{drive_letter}:\\");
        if !Path::new(&drive_root).exists() {
            continue;
        }

        for vdf_rel in STEAM_LIBRARY_VDF_RELATIVE_PATHS {
            let vdf_path: PathBuf = PathBuf::from(&drive_root).join(vdf_rel);
            if let Ok(content) = fs::read_to_string(&vdf_path) {
                for parsed_root in parse_steam_library_folders_vdf(&content) {
                    let key: String = parsed_root.to_string_lossy().to_ascii_lowercase();
                    if seen_roots.insert(key) {
                        steam_roots.push(parsed_root);
                    }
                }
            }
        }

        for root_rel in STEAM_ROOT_CANDIDATES {
            let candidate_root: PathBuf = PathBuf::from(&drive_root).join(root_rel);
            let key: String = candidate_root.to_string_lossy().to_ascii_lowercase();
            if seen_roots.insert(key) {
                steam_roots.push(candidate_root);
            }
        }
    }

    for root in steam_roots {
        checked = checked.saturating_add(1);
        let exe_candidate: PathBuf = root.join(STEAM_COMMON_SUFFIX);
        if exe_candidate.is_file() {
            let display: String = normalize_display_path(base_dir, &exe_candidate);
            return AutoDetectResponse {
                found: Some(display),
                checked_count: checked,
            };
        }
    }

    AutoDetectResponse {
        found: None,
        checked_count: checked,
    }
}
