use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Identifiers for the 3 built-in client DLLs embedded directly into `DH_ClientLauncher_JA.exe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinDllId {
    EosFixPlanA,
    EosFixPlanB,
    VoiceClient,
}

/// Mutually exclusive selection for the built-in EOS reconnect fix DLL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinEosFixPlan {
    None,
    PlanA,
    #[default]
    PlanB,
}

/// Persistent toggles for the built-in embedded client MOD DLLs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinDllConfig {
    #[serde(default)]
    pub eos_fix_plan: BuiltinEosFixPlan,
    #[serde(default)]
    pub voice_chat: bool,
}

/// Read-only metadata for a built-in embedded DLL exposed to the frontend UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltinDllMeta {
    pub id: String,
    pub label: String,
    pub file_name: String,
    pub description: String,
    pub sha256: String,
    pub raw_bytes: usize,
}

/// Verified on-disk cache path for an extracted built-in client DLL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedBuiltinDll {
    pub id: BuiltinDllId,
    pub label: &'static str,
    pub path: PathBuf,
    pub sha256: &'static str,
    pub cache_hit: bool,
}
