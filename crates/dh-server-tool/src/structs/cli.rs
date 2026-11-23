use crate::structs::languages::CliMsgCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebConsoleSetupConfig {
    pub web_console_port: u16,
    pub console_patch_port: u16,
    pub auto_broadcast: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapOption {
    pub key: &'static str,
    pub id: &'static str,
    pub label_code: CliMsgCode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ServerSettings {
    pub max_players: u32,
    pub days_before_blizzard: u32,
    pub day_minutes: u32,
    pub predator_damage: f64,
    pub cold_intensity: f64,
    pub hunger_rate: f64,
    pub coal_burn_rate: f64,
    pub thralls: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchConfig {
    pub map_id: &'static str,
    pub map_label_code: CliMsgCode,
    pub settings: ServerSettings,
    pub selected_option_numbers: Vec<u32>,
    pub selected_option_labels: Vec<String>,
    pub ue_map_argument: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LaunchPlan {
    pub web_console_port: u16,
    pub console_patch_port: u16,
    pub auto_broadcast: bool,
    pub server_ports: [u16; 2],
    pub map_id: &'static str,
    pub map_label_code: CliMsgCode,
    pub settings: ServerSettings,
    pub selected_option_numbers: Vec<u32>,
    pub selected_option_labels: Vec<String>,
    pub ue_map_argument: String,
}

pub(crate) struct IntegerPromptSpec {
    pub(crate) label_code: CliMsgCode,
    pub(crate) min: u32,
    pub(crate) max: u32,
    pub(crate) default_value: u32,
}

pub(crate) struct FloatPromptSpec {
    pub(crate) label_code: CliMsgCode,
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) default_value: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModSelectionStepResult {
    pub added_labels: Vec<&'static str>,
    pub duplicate_labels: Vec<&'static str>,
    pub conflict_notices: Vec<String>,
    pub invalid_tokens: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModSelectionCommand {
    Confirm,
    Cleared,
    EmptyInvalid,
    Processed(ModSelectionStepResult),
}
