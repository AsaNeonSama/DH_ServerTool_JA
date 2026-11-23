use crate::structs::cli::MapOption;
use crate::structs::languages::CliMsgCode;

pub const MOD_MENU_COLS: usize = 4;
pub const MOD_MENU_COL_WIDTH: usize = 27;

pub const DEFAULT_MAX_PLAYERS: u32 = 8;
pub const DEFAULT_DAYS_BEFORE_BLIZZARD: u32 = 3;
pub const DEFAULT_DAY_MINUTES: u32 = 9;
pub const DEFAULT_PREDATOR_DAMAGE: f64 = 1.0;
pub const DEFAULT_COLD_INTENSITY: f64 = 1.0;
pub const DEFAULT_HUNGER_RATE: f64 = 1.0;
pub const DEFAULT_COAL_BURN_RATE: f64 = 1.0;
pub const DEFAULT_THRALLS: u32 = 2;

pub const MAP_OPTIONS: [MapOption; 3] = [
    MapOption {
        key: "1",
        id: "Departure_Persistent",
        label_code: CliMsgCode::MapLabel1,
    },
    MapOption {
        key: "2",
        id: "Expanse_Persistent",
        label_code: CliMsgCode::MapLabel2,
    },
    MapOption {
        key: "3",
        id: "Approach_Persistent",
        label_code: CliMsgCode::MapLabel3,
    },
];
