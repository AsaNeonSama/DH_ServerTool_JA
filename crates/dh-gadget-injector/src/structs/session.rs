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

/// Parameters for spawning a target executable in suspended state, attaching Frida Gadget
/// at OEP, waiting for the `INIT_OK` handshake, and tracking the active session.
#[derive(Clone)]
pub struct InjectLaunchRequest {
    pub exe_path: PathBuf,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub mods: Vec<PatchMod>,
    pub tokens: HashMap<String, String>,
    pub patch_log_path: PathBuf,
    pub round: u64,
    pub init_timeout: Duration,
    pub cache_dir_override: Option<PathBuf>,
    pub on_event: Option<EventCallback>,
}

/// Owns the spawned game server process, its kill-on-close Job Object, the loopback
/// NDJSON log server, and the per-round temporary staging directory.
///
/// Dropping this session closes the Job Object (immediately terminating the child
/// process if still running) and cleans up the per-round staging directory.
#[derive(Debug)]
pub struct ActiveInjectionSession {
    pub(crate) pid: u32,
    pub(crate) process: SpawnedProcess,
    // Field order matters for Rust Drop: drop JobObject BEFORE RoundTempDir so Windows
    // terminates the server and releases the file lock on gadget.dll before dir removal.
    pub(crate) log_server: Option<LogServerHandle>,
    pub(crate) job: Option<JobObject>,
    pub(crate) round_dir: Option<RoundTempDir>,
    pub(crate) stub_info: InjectedStubInfo,
}
