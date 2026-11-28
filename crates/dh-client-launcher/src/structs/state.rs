use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use dh_gadget_injector::ActiveInjectionSession;
use serde::{Deserialize, Serialize};

use crate::structs::builtin_dlls::BuiltinDllMeta;
use crate::structs::config::{FileStatuses, LauncherConfig};
use crate::structs::launcher::{LauncherLogEvent, ProcessSnapshot};

/// Result sent back from the detached client launch Tokio task.
pub type LaunchOutcome = Result<ProcessSnapshot, String>;

/// Thread-safe bounded ring buffer of structured launcher logs retained across `WebView` reloads.
pub type LogBuffer = Arc<Mutex<VecDeque<LauncherLogEvent>>>;

/// Outcome reported from the dedicated named-mutex acquisition thread.
pub type MutexAcquireOutcome = Result<bool, String>;

/// Result of reconciling tracked launcher state against an observed OS-wide game process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileOutcome {
    pub snapshot: ProcessSnapshot,
    pub watch_target: Option<(u64, u32)>,
    pub changed: bool,
}

/// RAII owner of the OS-wide single-instance application mutex (`Local\DH_ClientLauncher_JA_AppInstance`).
/// Kept alive for the entire lifetime of `run()` so a second launcher process exits immediately.
pub struct AppInstanceGuard {
    pub(crate) handle: isize,
}

/// RAII owner of a Windows named mutex acquired on a dedicated owning OS thread so the lock can
/// safely be held across Tokio `.await` points without violating Win32 mutex thread affinity.
pub struct NamedMutexGuard {
    pub(crate) release_tx: Option<SyncSender<()>>,
    pub(crate) owner_thread: Option<JoinHandle<()>>,
}

/// Active process tracking guard owned by [`AppState`].
#[derive(Default)]
pub struct TrackedProcess {
    pub generation: u64,
    pub launching: bool,
    pub snapshot: ProcessSnapshot,
    pub session: Option<ActiveInjectionSession>,
    pub external_pid: Option<u32>,
}

/// Shared Tauri application state for configuration, buffered logs, and game process tracking.
pub struct AppState {
    pub base_dir: PathBuf,
    pub config: Mutex<LauncherConfig>,
    pub tracked: Arc<Mutex<TrackedProcess>>,
    pub logs: LogBuffer,
}

/// Full launcher state payload sent to the frontend UI on startup, reload, or config update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherStateResponse {
    pub config: LauncherConfig,
    pub statuses: FileStatuses,
    pub process: ProcessSnapshot,
    pub logs: Vec<LauncherLogEvent>,
    pub builtin_meta: Vec<BuiltinDllMeta>,
    pub license_url: String,
    pub version: String,
}
