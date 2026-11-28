pub const CONFIG_RELATIVE_PATH: &str = "./launcher_config.json";
pub const MAX_CONFIG_FILE_BYTES: u64 = 1024 * 1024;
pub const MIN_VALID_PORT: u16 = 1024;
pub const MAX_VALID_PORT: u16 = 65535;
pub const MIN_RESTART_WAIT_SECONDS: u64 = 1;
pub const MAX_SAFE_INT_F64: f64 = 9_007_199_254_740_991.0;

pub const DEFAULT_SERVER_PORTS: [u16; 2] = [7895, 7897];
pub const DEFAULT_SWITCH_PORT_ON_RESTART: bool = true;
pub const DEFAULT_CONSOLE_PATCH_PORT: u16 = 7893;
pub const DEFAULT_RESTART_WAIT_SECONDS: u64 = 10;
pub const DEFAULT_AUTO_BROADCAST: bool = true;
pub const DEFAULT_SERVER_EXECUTABLE: &str = "./DreadHungerServer.exe";
pub const DEFAULT_SHIPPING_EXECUTABLE: &str =
    "./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe";
pub const DEFAULT_SHIPPING_PROCESS_NAME: &str = "DreadHungerServer-Win64-Shipping.exe";
