use crate::structs::languages::ServerMsgCode;
use crate::structs::server::MapOption;

pub const PATCH_RUNTIME_LOG_PREFIX: &str = "patch_runtime_";
pub const PATCH_RUNTIME_LOG_EXTENSION: &str = ".log";
pub const MAX_PATCH_RUNTIME_LOG_FILES: usize = 3;
pub const UE_PROJECT_NAME: &str = "DreadHunger";
pub const LEGACY_GADGET_DLL_FILENAME: &str = "frida.dll";
pub const SERVER_CHECK_INTERVAL_MS: u64 = 1_000;
pub const STARTUP_RETRY_DELAY_MS: u64 = 5_000;
pub const UDP_MISSING_STREAK_LIMIT: u32 = 3;
pub const INJECTION_INIT_TIMEOUT_SECS: u64 = 30;
pub const DETACH_EXIT_GRACE_MS: u64 = 500;

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
        label_code: ServerMsgCode::MapLabel1,
    },
    MapOption {
        key: "2",
        id: "Expanse_Persistent",
        label_code: ServerMsgCode::MapLabel2,
    },
    MapOption {
        key: "3",
        id: "Approach_Persistent",
        label_code: ServerMsgCode::MapLabel3,
    },
];
