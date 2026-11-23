use std::borrow::Cow;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs::{self, File, ReadDir};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::error::{InjectorError, Result};

/// Embedded zstd-22 compressed `frida-gadget-17.27.4-windows-x86_64.dll`.
pub const EMBEDDED_GADGET_ZST: &[u8] = include_bytes!(env!("DH_FRIDA_GADGET_ZST_PATH"));
/// Frida Gadget version string embedded at build time.
pub const FRIDA_GADGET_VERSION: &str = env!("DH_FRIDA_GADGET_VERSION");
/// Expected lowercase hex SHA-256 of the uncompressed Frida Gadget DLL.
pub const FRIDA_GADGET_SHA256: &str = env!("DH_FRIDA_GADGET_SHA256");

static ROUND_SEQ: AtomicU64 = AtomicU64::new(1);

fn sha256_file_hex(path: &Path) -> std::io::Result<String> {
    let mut file: File = File::open(path)?;
    let mut hasher: Sha256 = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    let digest: [u8; 32] = hasher.finalize().into();
    let mut out: String = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    Ok(out)
}

fn sha256_bytes_hex(bytes: &[u8]) -> String {
    let mut hasher: Sha256 = Sha256::new();
    hasher.update(bytes);
    let digest: [u8; 32] = hasher.finalize().into();
    let mut out: String = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

/// Returns the default candidate cache directories in priority order:
/// 1. `%LOCALAPPDATA%\DH_ServerTool_JA`
/// 2. `%TEMP%\DH_ServerTool_JA`
/// 3. `./.dh_cache`
#[must_use]
pub fn default_cache_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::with_capacity(3);
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        if !local_app_data.trim().is_empty() {
            candidates.push(PathBuf::from(local_app_data).join("DH_ServerTool_JA"));
        }
    }
    candidates.push(std::env::temp_dir().join("DH_ServerTool_JA"));
    candidates.push(PathBuf::from(".dh_cache"));
    candidates
}

fn to_normal_absolute_path(path: &Path) -> std::io::Result<PathBuf> {
    let abs: PathBuf = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    // Avoid `\\?\` extended-length prefixes where unnecessary so Frida Gadget's
    // internal DLL->config path resolution works uniformly.
    let s: Cow<'_, str> = abs.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        Ok(PathBuf::from(stripped))
    } else {
        Ok(abs)
    }
}

fn ensure_writable_dir(dir: &Path) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let abs: PathBuf = to_normal_absolute_path(dir)?;
    let probe: PathBuf = abs.join(format!(".write_probe_{}.tmp", std::process::id()));
    File::create(&probe)?;
    let _ = fs::remove_file(&probe);
    Ok(abs)
}

/// Removes stale `run_<pid>_*` temporary round directories left behind by crashed
/// launcher processes whose PID is no longer alive.
pub fn sweep_stale_run_dirs(cache_root: &Path) {
    let Ok(entries): std::io::Result<ReadDir> = fs::read_dir(cache_root) else {
        return;
    };
    let my_pid: u32 = std::process::id();
    for entry in entries.flatten() {
        let path: PathBuf = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name): Option<&str> = path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        let Some(rest): Option<&str> = name.strip_prefix("run_") else {
            continue;
        };
        let Some(pid_str): Option<&str> = rest.split('_').next() else {
            continue;
        };
        let Ok(owner_pid): std::result::Result<u32, std::num::ParseIntError> =
            pid_str.parse::<u32>()
        else {
            continue;
        };
        if owner_pid != my_pid && !dh_win32::is_process_alive(owner_pid) {
            let _ = fs::remove_dir_all(&path);
        }
    }
}

/// Sweeps stale `run_*` directories across the default cache locations.
pub fn sweep_default_stale_run_dirs() {
    for candidate in default_cache_candidates() {
        if candidate.is_dir() {
            sweep_stale_run_dirs(&candidate);
        }
    }
}

/// Manages the persistent SHA-256 verified Frida Gadget DLL cache and per-round
/// isolated execution directories (using NTFS hard links when on the same volume).
#[derive(Debug, Clone)]
pub struct GadgetCache {
    root_dir: PathBuf,
    cached_dll_path: PathBuf,
}

impl GadgetCache {
    /// Opens (or initializes) the Gadget DLL cache, optionally overriding the root directory.
    /// Also sweeps stale `run_*` directories from crashed previous launcher instances.
    pub fn open(override_root: Option<&Path>) -> Result<Self> {
        let root_dir: PathBuf = if let Some(custom) = override_root {
            ensure_writable_dir(custom)?
        } else {
            let mut chosen: Option<PathBuf> = None;
            for candidate in default_cache_candidates() {
                if let Ok(abs) = ensure_writable_dir(&candidate) {
                    chosen = Some(abs);
                    break;
                }
            }
            chosen.ok_or(InjectorError::NoWritableCacheDir)?
        };

        sweep_stale_run_dirs(&root_dir);

        let short_hash: &str = &FRIDA_GADGET_SHA256[..12.min(FRIDA_GADGET_SHA256.len())];
        let dll_name: String = format!("frida_gadget_{FRIDA_GADGET_VERSION}_{short_hash}.dll");
        let cached_dll_path: PathBuf = root_dir.join(dll_name);

        let valid_existing: bool = cached_dll_path.is_file()
            && sha256_file_hex(&cached_dll_path).is_ok_and(|h: String| h == FRIDA_GADGET_SHA256);

        if !valid_existing {
            let dll_bytes: Vec<u8> = zstd::stream::decode_all(EMBEDDED_GADGET_ZST)
                .map_err(|e: std::io::Error| InjectorError::Decompress(e.to_string()))?;
            let actual_hash: String = sha256_bytes_hex(&dll_bytes);
            if actual_hash != FRIDA_GADGET_SHA256 {
                return Err(InjectorError::Decompress(format!(
                    "decompressed DLL SHA-256 mismatch: expected {FRIDA_GADGET_SHA256}, got {actual_hash}"
                )));
            }

            let mut suffix: [u8; 4] = [0u8; 4];
            rand::rngs::OsRng.fill_bytes(&mut suffix);
            let tmp_name: String = format!(
                "frida_gadget_{FRIDA_GADGET_VERSION}_{short_hash}.{}.{:02x}{:02x}{:02x}{:02x}.tmp",
                std::process::id(),
                suffix[0],
                suffix[1],
                suffix[2],
                suffix[3]
            );
            let tmp_path: PathBuf = root_dir.join(tmp_name);

            {
                let mut f: File = File::create(&tmp_path)?;
                f.write_all(&dll_bytes)?;
                f.sync_all()?;
            }

            if let Err(rename_err) = fs::rename(&tmp_path, &cached_dll_path) {
                // Another instance may have raced and placed the identical verified DLL.
                let now_valid: bool = cached_dll_path.is_file()
                    && sha256_file_hex(&cached_dll_path)
                        .is_ok_and(|h: String| h == FRIDA_GADGET_SHA256);
                let _ = fs::remove_file(&tmp_path);
                if !now_valid {
                    return Err(InjectorError::Io(rename_err));
                }
            }
        }

        Ok(Self {
            root_dir,
            cached_dll_path,
        })
    }

    #[must_use]
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    #[must_use]
    pub fn cached_dll_path(&self) -> &Path {
        &self.cached_dll_path
    }

    /// Creates an isolated per-round temp directory `run_<pid>_<round>/` containing:
    /// - `gadget.dll` (NTFS hard-linked to the cached DLL, falling back to file copy)
    /// - `gadget.config` (`runtime: "qjs"`, `type: "script"`, `path: "bundle.js"`, `on_load: "resume"`)
    /// - `bundle.js` (UTF-8 aggregated script)
    pub fn prepare_round_dir(&self, round: u64, bundle_js: &str) -> Result<RoundTempDir> {
        let pid: u32 = std::process::id();
        let seq: u64 = ROUND_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir_name: String = if seq == 1 {
            format!("run_{pid}_{round}")
        } else {
            format!("run_{pid}_{round}_{seq}")
        };
        let dir: PathBuf = self.root_dir.join(dir_name);
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
        fs::create_dir_all(&dir)?;

        let gadget_dll_path: PathBuf = dir.join("gadget.dll");
        if fs::hard_link(&self.cached_dll_path, &gadget_dll_path).is_err() {
            fs::copy(&self.cached_dll_path, &gadget_dll_path)?;
        }

        let config_path: PathBuf = dir.join("gadget.config");
        let config_json: serde_json::Value = serde_json::json!({
            "runtime": "qjs",
            "interaction": {
                "type": "script",
                "path": "bundle.js",
                "on_load": "resume"
            }
        });
        fs::write(&config_path, serde_json::to_string_pretty(&config_json)?)?;

        let bundle_js_path: PathBuf = dir.join("bundle.js");
        fs::write(&bundle_js_path, bundle_js.as_bytes())?;

        Ok(RoundTempDir {
            dir,
            gadget_dll_path,
            config_path,
            bundle_js_path,
            cleaned: false,
        })
    }
}

/// RAII owner for a single match round's `run_<pid>_<round>/` staging directory.
#[derive(Debug)]
pub struct RoundTempDir {
    dir: PathBuf,
    gadget_dll_path: PathBuf,
    config_path: PathBuf,
    bundle_js_path: PathBuf,
    cleaned: bool,
}

impl RoundTempDir {
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn gadget_dll_path(&self) -> &Path {
        &self.gadget_dll_path
    }

    #[must_use]
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    #[must_use]
    pub fn bundle_js_path(&self) -> &Path {
        &self.bundle_js_path
    }

    /// Best-effort recursive removal with brief retries so Windows finishes unmapping
    /// `gadget.dll` immediately after process termination.
    pub fn cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        self.cleaned = true;
        for attempt in 0u8..5u8 {
            if !self.dir.exists() || fs::remove_dir_all(&self.dir).is_ok() {
                return;
            }
            if attempt < 4 {
                std::thread::sleep(Duration::from_millis(60));
            }
        }
    }
}

impl Drop for RoundTempDir {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_extracts_hardlinks_and_cleans_up_in_japanese_path() {
        let base =
            std::env::temp_dir().join(format!("dh_cache_テスト日本語_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);

        // Create a fake stale directory for an impossible dead PID
        let stale = base.join("run_99999999_1");
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join("dummy.txt"), b"stale").unwrap();

        let cache = GadgetCache::open(Some(&base)).expect("open Japanese cache path");
        assert!(!stale.exists(), "stale run dir should be swept on open");
        assert!(cache.cached_dll_path().is_file());

        let round_dir_path;
        {
            let round = cache
                .prepare_round_dir(1, "console.log('hello');")
                .expect("prepare round dir");
            round_dir_path = round.dir().to_path_buf();
            assert!(round.gadget_dll_path().is_file());
            assert!(round.config_path().is_file());
            assert!(round.bundle_js_path().is_file());

            let cfg: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(round.config_path()).unwrap()).unwrap();
            assert_eq!(cfg["runtime"], "qjs");
            assert_eq!(cfg["interaction"]["type"], "script");
            assert_eq!(cfg["interaction"]["path"], "bundle.js");
            assert_eq!(cfg["interaction"]["on_load"], "resume");
        }
        assert!(
            !round_dir_path.exists(),
            "RoundTempDir must clean itself up on Drop"
        );

        let _ = fs::remove_dir_all(&base);
    }
}
