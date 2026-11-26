use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub server_ports: [u16; 2],
    pub switch_port_on_restart: bool,
    pub console_patch_port: u16,
    pub web_console_port: u16,
    pub check_interval_ms: u64,
    pub restart_wait_seconds: u64,
    pub auto_broadcast_default: bool,
    pub server_executable: String,
    pub shipping_executable: String,
    pub shipping_process_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLoadOutcome {
    Loaded,
    CreatedDefault,
    CreateFailed,
    LoadFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub config: AppConfig,
    pub outcome: ConfigLoadOutcome,
}
