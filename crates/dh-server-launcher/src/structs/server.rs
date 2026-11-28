use crate::structs::languages::ServerMsgCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchLogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapOption {
    pub key: &'static str,
    pub id: &'static str,
    pub label_code: ServerMsgCode,
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
    pub map_label_code: ServerMsgCode,
    pub settings: ServerSettings,
    pub selected_content_option_numbers: Vec<u32>,
    pub selected_content_option_labels: Vec<String>,
    pub selected_option_numbers: Vec<u32>,
    pub selected_option_labels: Vec<String>,
    pub ue_map_argument: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LaunchPlan {
    pub console_patch_port: u16,
    pub server_ports: [u16; 2],
    pub switch_port_on_restart: bool,
    pub map_id: &'static str,
    pub map_label_code: ServerMsgCode,
    pub settings: ServerSettings,
    pub selected_content_option_numbers: Vec<u32>,
    pub selected_content_option_labels: Vec<String>,
    pub selected_option_numbers: Vec<u32>,
    pub selected_option_labels: Vec<String>,
    pub ue_map_argument: String,
}
