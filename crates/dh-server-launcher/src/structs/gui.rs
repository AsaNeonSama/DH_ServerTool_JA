use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::structs::config::{AppConfig, ConfigLoadOutcome};
use crate::structs::languages::ApiCode;
use crate::structs::mods::{ExtractedServerDll, ResolvedPatchScript};
use crate::structs::server::LaunchPlan;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapOptionDto {
    pub key: String,
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchSettingsDto {
    pub max_players: u32,
    pub days_before_blizzard: u32,
    pub day_minutes: u32,
    pub predator_damage: f64,
    pub cold_intensity: f64,
    pub hunger_rate: f64,
    pub coal_burn_rate: f64,
    pub thralls: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultModDto {
    pub id: String,
    pub file_name: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModOptionDto {
    pub id: String,
    pub number: u32,
    pub label: String,
    pub kind: String,
    pub requirement_badge: Option<String>,
    pub requirement_detail: Option<String>,
    pub description: Option<String>,
    pub scripts: Vec<String>,
    pub conflicts_with: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictPairDto {
    pub a_id: String,
    pub a_number: u32,
    pub a_label: String,
    pub b_id: String,
    pub b_number: u32,
    pub b_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModCatalogDto {
    pub default_mods: Vec<DefaultModDto>,
    pub client_required_mods: Vec<ModOptionDto>,
    pub practical_mods: Vec<ModOptionDto>,
    pub fun_mods: Vec<ModOptionDto>,
    pub conflict_pairs: Vec<ConflictPairDto>,
}

pub struct GuiRoundSpawnSpec<'a> {
    pub config: &'a AppConfig,
    pub ue_map_argument: &'a str,
    pub port: u16,
    pub round: u64,
    pub patches: &'a [ResolvedPatchScript],
    pub dlls: &'a [ExtractedServerDll],
}

pub struct GuiSupervisorBootstrap {
    pub initial_plan: LaunchPlan,
    pub initial_patches: Vec<ResolvedPatchScript>,
    pub initial_dlls: Vec<ExtractedServerDll>,
    pub config: AppConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleChoiceDto {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleStatusSpecDto {
    pub value: String,
    pub label: String,
    pub is_toggle: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminConsoleCatalogDto {
    pub roles: Vec<ConsoleChoiceDto>,
    pub command_types: Vec<ConsoleChoiceDto>,
    pub summons: Vec<ConsoleChoiceDto>,
    pub spell_charges: Vec<ConsoleChoiceDto>,
    pub blizzard_intensities: Vec<ConsoleChoiceDto>,
    pub slomos: Vec<ConsoleChoiceDto>,
    pub statuses: Vec<ConsoleStatusSpecDto>,
    pub items: Vec<ConsoleChoiceDto>,
    pub item_counts: Vec<i64>,
    pub storages: Vec<ConsoleChoiceDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleDraftDto {
    pub operation: String,
    pub target_mode: String,
    pub target_role: String,
    pub custom_target: String,
    pub auto_broadcast: bool,
    pub console_order: String,
    pub summon_target: String,
    pub is_thrall: String,
    pub spell_charge: String,
    pub blizzard_intensity: String,
    pub slomo: String,
    pub god: String,
    pub end_match: String,
    pub raw_command: String,
    pub status_prop: String,
    pub status_toggle: String,
    pub status_number: String,
    pub item_filter: String,
    pub item_path: String,
    pub item_count: String,
    pub storage_limit: String,
    pub chat_broadcast: bool,
    pub chat_message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDraftDto {
    pub map_id: String,
    pub settings: MatchSettingsDto,
    pub selected_content_numbers: Vec<u32>,
    pub selected_option_numbers: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiSessionDraftDto {
    pub active_tab: String,
    pub console_draft: ConsoleDraftDto,
    pub config_draft: AppConfig,
    pub auto_restart_watchdog: bool,
    pub match_draft: MatchDraftDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AdminConsoleActionRequestDto {
    ConsoleCommand {
        #[serde(rename = "playerName")]
        player_name: String,
        command: String,
        #[serde(rename = "autoBroadcast")]
        auto_broadcast: bool,
    },
    SetHuman {
        #[serde(rename = "playerName")]
        player_name: String,
        #[serde(rename = "propName")]
        prop_name: String,
        value: String,
        #[serde(rename = "autoBroadcast")]
        auto_broadcast: bool,
    },
    AddItem {
        #[serde(rename = "playerName")]
        player_name: String,
        #[serde(rename = "itemName")]
        item_name: String,
        count: String,
        #[serde(rename = "autoBroadcast")]
        auto_broadcast: bool,
    },
    SetStorage {
        #[serde(rename = "playerName")]
        player_name: String,
        limit: String,
        #[serde(rename = "autoBroadcast")]
        auto_broadcast: bool,
    },
    SendThrallMsg {
        #[serde(rename = "playerName")]
        player_name: String,
        msg: String,
        broadcast: bool,
    },
    KickPlayer {
        #[serde(rename = "playerName")]
        player_name: String,
        #[serde(rename = "autoBroadcast")]
        auto_broadcast: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminConsoleActionResultDto {
    pub ok: bool,
    pub status: u16,
    pub code: ApiCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerPhase {
    Stopped,
    Launching,
    Running,
    Countdown,
    Restarting,
    Stopping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchProgressStatus {
    Pending,
    Loaded,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchProgressCategory {
    Default,
    ClientRequired,
    ServerOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchProgressItemDto {
    pub index: usize,
    pub id: String,
    pub file_name: String,
    pub label: String,
    pub category: PatchProgressCategory,
    pub status: PatchProgressStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerMonitorStateDto {
    pub phase: ServerPhase,
    pub pid: Option<u32>,
    pub active_port: Option<u16>,
    pub next_port: Option<u16>,
    pub switch_port_on_restart: bool,
    pub server_ports: [u16; 2],
    pub udp_confirmed: bool,
    pub round: u32,
    pub map_id: Option<String>,
    pub map_label: Option<String>,
    pub active_mod_count: usize,
    pub injected_count: usize,
    pub total_patch_count: usize,
    pub patch_progress: Vec<PatchProgressItemDto>,
    pub countdown_seconds: Option<u64>,
    pub current_patch_log_file: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleLogEntryDto {
    pub id: u64,
    pub time: String,
    pub category: String,
    pub tone: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerLogLevel {
    Info,
    Success,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerLogEntryDto {
    pub id: u64,
    pub time: String,
    pub level: ServerLogLevel,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiSnapshot {
    pub app_version: String,
    pub license_url: String,
    pub config: AppConfig,
    pub config_outcome: String,
    pub config_notice: Option<String>,
    pub server_exe_exists: bool,
    pub legacy_frida_dll_detected: bool,
    pub auto_restart_watchdog: bool,
    pub console_catalog: AdminConsoleCatalogDto,
    pub maps: Vec<MapOptionDto>,
    pub default_match_settings: MatchSettingsDto,
    pub default_mods: Vec<DefaultModDto>,
    pub client_required_mods: Vec<ModOptionDto>,
    pub practical_mods: Vec<ModOptionDto>,
    pub fun_mods: Vec<ModOptionDto>,
    pub conflict_pairs: Vec<ConflictPairDto>,
    pub ui_draft: UiSessionDraftDto,
    pub server_status: ServerMonitorStateDto,
    pub console_logs: Vec<ConsoleLogEntryDto>,
    pub server_logs: Vec<ServerLogEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSaveResultDto {
    pub config: AppConfig,
    pub auto_restart_watchdog: bool,
    pub server_exe_exists: bool,
    pub legacy_frida_dll_detected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickedFileDto {
    pub path: String,
    pub process_name: String,
    pub exists: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchServerRequestDto {
    pub map_id: String,
    pub settings: MatchSettingsDto,
    pub selected_content_numbers: Vec<u32>,
    pub selected_option_numbers: Vec<u32>,
    pub auto_restart_watchdog: bool,
}

#[derive(Debug, Clone)]
pub enum ServerControlCommand {
    RestartNow,
    CancelCountdownForReconfigure,
    UpdateNextPlan(LaunchPlan),
}

pub struct ActiveSupervisorHandle {
    pub cancel: CancellationToken,
    pub cmd_tx: mpsc::UnboundedSender<ServerControlCommand>,
    pub task: JoinHandle<()>,
}

pub struct GuiSharedData {
    pub config: AppConfig,
    pub config_outcome: ConfigLoadOutcome,
    pub ui_draft: UiSessionDraftDto,
    pub server_status: ServerMonitorStateDto,
    pub console_logs: VecDeque<ConsoleLogEntryDto>,
    pub server_logs: VecDeque<ServerLogEntryDto>,
    pub next_log_id: u64,
}

#[derive(Clone)]
pub struct GuiAppState {
    pub shared: Arc<Mutex<GuiSharedData>>,
    pub supervisor_handle: Arc<AsyncMutex<Option<ActiveSupervisorHandle>>>,
}
