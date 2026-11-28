pub mod builtin_dlls;
pub mod config;
pub mod discovery;
pub mod launcher;
pub mod state;

pub use builtin_dlls::{
    BuiltinDllConfig, BuiltinDllId, BuiltinDllMeta, BuiltinEosFixPlan, ExtractedBuiltinDll,
};
pub use config::{FileStatuses, LauncherConfig};
pub use discovery::AutoDetectResponse;
pub use launcher::{LauncherLogEvent, PreparedLaunchInputs, ProcessLifecycle, ProcessSnapshot};
pub use state::{AppState, LaunchOutcome, LauncherStateResponse, LogBuffer, TrackedProcess};
