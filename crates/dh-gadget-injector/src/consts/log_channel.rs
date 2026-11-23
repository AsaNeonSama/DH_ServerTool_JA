pub const DEFAULT_PATCH_LOG_PATH: &str = "./patch_runtime.log";
pub const DEFAULT_INIT_TIMEOUT_SECS: u64 = 30;
pub const EXIT_POLL_INTERVAL_MS: u64 = 200;
pub const SESSION_NONCE_BYTES: usize = 16;

pub const PACKET_TYPE_MOD_OK: &str = "MOD_OK";
pub const PACKET_TYPE_LOG: &str = "LOG";
pub const PACKET_TYPE_INIT_OK: &str = "INIT_OK";
pub const PACKET_TYPE_INIT_ERR: &str = "INIT_ERR";
