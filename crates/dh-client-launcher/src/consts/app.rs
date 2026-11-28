pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_LICENSE_URL: &str = "https://github.com/AsaNeonSama/DH_Tool_JA/blob/main/LICENSE";
pub const APP_LICENSE_TEXT: &str = include_str!("../../../../LICENSE");
pub const MAIN_WINDOW_LABEL: &str = "main";
pub const MAIN_WINDOW_TITLE: &str = "DH_ClientLauncher_JA — Dread Hunger クライアントランチャー";
pub const MAX_BUFFERED_LOGS: usize = 600;
pub const PROCESS_POLL_INTERVAL_MS: u64 = 600;
pub const CLIENT_PATCH_LOG_RELATIVE_PATH: &str = "./client_patch_runtime.log";
pub const CLIENT_INIT_TIMEOUT_SECS: u64 = 30;

pub const DREAD_HUNGER_STEAM_APP_ID: &str = "1418630";
pub const ENV_STEAM_APP_ID: &str = "SteamAppId";
pub const ENV_STEAM_GAME_ID: &str = "SteamGameId";

pub const CLIENT_APP_INSTANCE_MUTEX_NAME: &str = r"Local\DH_ClientLauncher_JA_AppInstance";
pub const CLIENT_LAUNCH_MUTEX_NAME: &str = r"Local\DH_ClientLauncher_JA_LaunchGuard";
pub const LAUNCH_MUTEX_WAIT_MS: u32 = 250;
pub const KNOWN_GAME_EXE_NAMES: &[&str] = &["DreadHunger-Win64-Shipping.exe", "DreadHunger.exe"];
