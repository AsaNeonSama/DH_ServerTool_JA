use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dh_win32::{JobObject, SpawnedProcess};

use crate::structs::cache::RoundTempDir;
use crate::structs::log_channel::LogServerHandle;
use crate::structs::oep_inject::InjectedStubInfo;

/// Single MOD script entry to be aggregated into the per-round Gadget bundle.
#[derive(Debug, Clone)]
pub struct PatchMod {
    pub id: String,
    pub file_name: String,
    pub label: String,
    pub source: String,
}

/// Severity level for runtime JS logs emitted by injected MOD scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// Real-time lifecycle and log events streamed from Frida Gadget over the loopback NDJSON channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectorEvent {
    ModLoaded {
        file_name: String,
    },
    Log {
        level: LogLevel,
        file_name: String,
        message: String,
    },
    InitOk,
    InitError {
        file_name: String,
        message: String,
    },
    ChannelClosed,
}

/// Callback invoked asynchronously whenever the Gadget bundle streams a status or log event.
pub type EventCallback = Arc<dyn Fn(InjectorEvent) + Send + Sync>;

/// Result of early-injecting a single DLL via `LoadLibraryW` inside the OEP bootstrap stub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DllLoadResult {
    pub path: PathBuf,
    /// Non-zero remote `HMODULE` returned by `LoadLibraryW`, or `None` if `LoadLibraryW` returned `NULL`.
    pub module_handle: Option<u64>,
}

/// Real-time per-DLL early-injection event emitted as the OEP stub completes each `LoadLibraryW`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DllInjectionEvent {
    Loaded {
        index: usize,
        total: usize,
        path: PathBuf,
        module_handle: u64,
    },
    Failed {
        index: usize,
        total: usize,
        path: PathBuf,
    },
}

/// Callback invoked as each user-configured DLL finishes loading in the target process at OEP.
pub type DllEventCallback = Arc<dyn Fn(DllInjectionEvent) + Send + Sync>;

/// Parameters for spawning a target executable in suspended state, optionally early-injecting
/// an ordered list of DLLs and/or Frida Gadget JS MODs at OEP, and tracking the active session.
#[derive(Clone)]
pub struct InjectLaunchRequest {
    pub exe_path: PathBuf,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env_overrides: Vec<(String, String)>,
    pub dlls: Vec<PathBuf>,
    pub mods: Vec<PatchMod>,
    pub tokens: HashMap<String, String>,
    pub patch_log_path: PathBuf,
    pub round: u64,
    pub init_timeout: Duration,
    pub cache_dir_override: Option<PathBuf>,
    pub create_console: bool,
    pub attach_job_object: bool,
    pub fail_on_dll_load_error: bool,
    pub on_event: Option<EventCallback>,
    pub on_dll_event: Option<DllEventCallback>,
}

/// Internal grouped parameters for OEP DLL/JS completion waiters.
pub(crate) struct DllWaitConfig<'a> {
    pub(crate) user_dlls: &'a [PathBuf],
    pub(crate) init_timeout: Duration,
    pub(crate) fail_on_dll_load_error: bool,
    pub(crate) on_dll_event: Option<&'a DllEventCallback>,
}

/// Owns the spawned target process handle, optional kill-on-close Job Object, optional loopback
/// NDJSON log server, optional per-round Gadget staging directory, and DLL load results.
///
/// When `attach_job_object` was `true` (server default), dropping this session closes the
/// Job Object (immediately terminating the child process if still running) and removes the
/// per-round staging directory.
/// When `attach_job_object` was `false` (GUI client default), dropping or detaching this session
/// leaves the running process alive.
#[derive(Debug)]
pub struct ActiveInjectionSession {
    pub(crate) pid: u32,
    pub(crate) process: Option<SpawnedProcess>,
    // Field order matters for Rust Drop: drop JobObject BEFORE RoundTempDir so Windows
    // terminates the child and releases the file lock on gadget.dll before dir removal.
    pub(crate) log_server: Option<LogServerHandle>,
    pub(crate) job: Option<JobObject>,
    pub(crate) round_dir: Option<RoundTempDir>,
    pub(crate) stub_info: InjectedStubInfo,
    pub(crate) dll_results: Vec<DllLoadResult>,
}
