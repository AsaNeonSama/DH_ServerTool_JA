use std::borrow::Cow;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs::{self, File, ReadDir};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rand::rngs::{SysError, SysRng};
use rand::TryRng;
use sha2::{Digest, Sha256};

use crate::consts::cache::{
    BUNDLE_JS_FILE_NAME, CACHE_APP_DIR_NAME, CLEANUP_RETRY_ATTEMPTS, CLEANUP_RETRY_DELAY_MS,
    FALLBACK_LOCAL_CACHE_DIR, GADGET_CONFIG_FILE_NAME, GADGET_DLL_FILE_NAME, RUN_DIR_PREFIX,
};
pub use crate::consts::cache::{EMBEDDED_GADGET_ZST, FRIDA_GADGET_SHA256, FRIDA_GADGET_VERSION};
use crate::error::{InjectorError, Result};
pub use crate::structs::cache::{GadgetCache, RoundTempDir};

static ROUND_SEQ: AtomicU64 = AtomicU64::new(1);
static PROBE_SEQ: AtomicU64 = AtomicU64::new(1);

fn digest_to_hex(digest: [u8; 32]) -> String {
    let mut out: String = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn sha256_file_hex(path: &Path) -> std::io::Result<String> {
    let mut file: File = File::open(path)?;
    let mut hasher: Sha256 = Sha256::new();
    let mut buf: [u8; 8192] = [0u8; 8192];
    loop {
        let read_len: usize = file.read(&mut buf)?;
        if read_len == 0 {
            break;
        }
        hasher.update(&buf[..read_len]);
    }
    let digest: [u8; 32] = hasher.finalize().into();
    Ok(digest_to_hex(digest))
}

fn sha256_bytes_hex(bytes: &[u8]) -> String {
    let mut hasher: Sha256 = Sha256::new();
    hasher.update(bytes);
    let digest: [u8; 32] = hasher.finalize().into();
    digest_to_hex(digest)
}

/// Returns the default candidate cache directories in priority order:
/// 1. `%LOCALAPPDATA%\DH_Tool_JA`
/// 2. `%TEMP%\DH_Tool_JA`
/// 3. `./.dh_cache`
#[must_use]
pub fn default_cache_candidates() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::with_capacity(3);
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        if !local_app_data.trim().is_empty() {
            candidates.push(PathBuf::from(local_app_data).join(CACHE_APP_DIR_NAME));
        }
    }
    candidates.push(std::env::temp_dir().join(CACHE_APP_DIR_NAME));
    candidates.push(PathBuf::from(FALLBACK_LOCAL_CACHE_DIR));
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
    let probe_seq: u64 = PROBE_SEQ.fetch_add(1, Ordering::Relaxed);
    let probe: PathBuf = abs.join(format!(
        ".write_probe_{}_{probe_seq}.tmp",
        std::process::id()
    ));
    let _file: File = File::create(&probe)?;
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
        let Some(rest): Option<&str> = name.strip_prefix(RUN_DIR_PREFIX) else {
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

        let short_hash: &str = FRIDA_GADGET_SHA256.get(..12).unwrap_or(FRIDA_GADGET_SHA256);
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
            SysRng
                .try_fill_bytes(&mut suffix)
                .map_err(|e: SysError| InjectorError::Io(std::io::Error::other(e.to_string())))?;
            let tmp_name: String = format!(
                "frida_gadget_{FRIDA_GADGET_VERSION}_{short_hash}.{}.{:02x}{:02x}{:02x}{:02x}.tmp",
                std::process::id(),
                suffix[0],
                suffix[1],
                suffix[2],
                suffix[3]
            );
            let tmp_path: PathBuf = root_dir.join(tmp_name);

            let write_res: std::io::Result<()> = (|| -> std::io::Result<()> {
                let mut f: File = File::create(&tmp_path)?;
                f.write_all(&dll_bytes)?;
                f.sync_all()?;
                Ok(())
            })();
            if let Err(err) = write_res {
                let _ = fs::remove_file(&tmp_path);
                return Err(InjectorError::Io(err));
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

        let gadget_dll_path: PathBuf = dir.join(GADGET_DLL_FILE_NAME);
        let config_path: PathBuf = dir.join(GADGET_CONFIG_FILE_NAME);
        let bundle_js_path: PathBuf = dir.join(BUNDLE_JS_FILE_NAME);

        // Wrap immediately so any error below cleans up the partially populated directory on Drop.
        let round_dir: RoundTempDir = RoundTempDir {
            dir,
            gadget_dll_path,
            config_path,
            bundle_js_path,
            cleaned: false,
        };

        if fs::hard_link(&self.cached_dll_path, &round_dir.gadget_dll_path).is_err() {
            fs::copy(&self.cached_dll_path, &round_dir.gadget_dll_path)?;
        }

        let config_json: serde_json::Value = serde_json::json!({
            "runtime": "qjs",
            "interaction": {
                "type": "script",
                "path": BUNDLE_JS_FILE_NAME,
                "on_load": "resume"
            }
        });
        fs::write(
            &round_dir.config_path,
            serde_json::to_string_pretty(&config_json)?,
        )?;
        fs::write(&round_dir.bundle_js_path, bundle_js.as_bytes())?;

        Ok(round_dir)
    }
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

    /// Disarms automatic directory removal on `Drop` (used when detaching from a still-running
    /// child process that keeps `gadget.dll` mapped; `sweep_stale_run_dirs` reclaims it later).
    pub(crate) fn disarm_cleanup(&mut self) {
        self.cleaned = true;
    }

    /// Best-effort recursive removal with brief retries so Windows finishes unmapping
    /// `gadget.dll` immediately after process termination.
    pub fn cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        self.cleaned = true;
        for attempt in 0u8..CLEANUP_RETRY_ATTEMPTS {
            if !self.dir.exists() || fs::remove_dir_all(&self.dir).is_ok() {
                return;
            }
            if attempt.saturating_add(1) < CLEANUP_RETRY_ATTEMPTS {
                std::thread::sleep(Duration::from_millis(CLEANUP_RETRY_DELAY_MS));
            }
        }
    }
}

impl Drop for RoundTempDir {
    fn drop(&mut self) {
        self.cleanup();
    }
}
