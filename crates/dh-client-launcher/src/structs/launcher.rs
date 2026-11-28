use std::path::PathBuf;

use dh_gadget_injector::PatchMod;
use serde::{Deserialize, Serialize};

use crate::structs::builtin_dlls::BuiltinDllId;

/// Lifecycle state of the game process tracked by the launcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProcessLifecycle {
    #[default]
    Idle,
    Injecting,
    Running,
    Exited,
}

/// Serializable snapshot of the tracked game process streamed to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSnapshot {
    pub status: ProcessLifecycle,
    pub pid: Option<u32>,
    pub exit_code: Option<u32>,
    pub mode_label: String,
}

/// Structured real-time log event emitted to the `WebView` over `launcher://log`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherLogEvent {
    pub timestamp: String,
    pub level: String,
    pub category: String,
    pub source: String,
    pub message: String,
}

/// Validated and resolved launch inputs prepared prior to spawning the client process.
#[derive(Debug, Clone)]
pub struct PreparedLaunchInputs {
    pub exe_path: PathBuf,
    pub args: Vec<String>,
    pub builtin_ids: Vec<BuiltinDllId>,
    pub mods: Vec<PatchMod>,
    pub env_overrides: Vec<(String, String)>,
    pub mode_label: String,
}
