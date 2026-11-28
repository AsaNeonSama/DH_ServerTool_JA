pub const PLAYER_NAME_LIST: [&str; 8] = [
    "Captain",
    "Chaplain",
    "Cook",
    "Doctor",
    "Engineer",
    "Hunter",
    "Marine",
    "Navigator",
];

pub const DEFAULT_TCP_TIMEOUT_MS: u64 = 1500;
pub const BONE_DAGGER_ITEM: &str =
    "/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C";

pub const MAX_LOG_TEXT_CHARS: usize = 64;
pub const HIGH_STAT_WARNING_THRESHOLD: f64 = 500.0;
pub const MAX_SAFE_INT_F64: f64 = 9_007_199_254_740_991.0;

pub mod status_code {
    pub const SUCCESS: u16 = 200;
    pub const BAD_REQUEST: u16 = 400;
    pub const SERVER_ERROR: u16 = 504;
    pub const INVALID_VALUE: u16 = 416;
    pub const PARTIAL_SUCCESS: u16 = 207;
}
