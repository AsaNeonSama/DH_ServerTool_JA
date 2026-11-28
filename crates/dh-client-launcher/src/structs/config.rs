use serde::{Deserialize, Serialize};

use crate::structs::builtin_dlls::BuiltinDllConfig;

/// Persistent client launcher configuration saved to `./launcher_config.json`.
///
/// Legacy fields from older config files (`launchArgs`, `dlls`, `scripts`, `jsScripts`)
/// are safely ignored during deserialization for backwards compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    pub exe_path: String,
    #[serde(default)]
    pub builtin: BuiltinDllConfig,
}

/// Live file-existence check results matching the current [`LauncherConfig`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatuses {
    pub exe_exists: bool,
}
