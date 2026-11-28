pub const DEFAULT_PATCH_LOG_PATH: &str = "./patch_runtime.log";
pub const DEFAULT_INIT_TIMEOUT_SECS: u64 = 30;
pub const EXIT_POLL_INTERVAL_MS: u64 = 200;
pub const DLL_POLL_INTERVAL_MS: u64 = 15;
pub const HANDSHAKE_POLL_INTERVAL_MS: u64 = 50;
pub const PROCESS_EXIT_WAIT_MS: u32 = 1000;
pub const SESSION_NONCE_BYTES: usize = 16;
pub const MAX_NDJSON_LINE_BYTES: usize = 1024 * 1024;
pub const MAX_CONCURRENT_LOG_CLIENTS: usize = 16;
pub const UNAUTH_FIRST_LINE_TIMEOUT_SECS: u64 = 5;

pub const PACKET_TYPE_MOD_OK: &str = "MOD_OK";
pub const PACKET_TYPE_LOG: &str = "LOG";
pub const PACKET_TYPE_INIT_OK: &str = "INIT_OK";
pub const PACKET_TYPE_INIT_ERR: &str = "INIT_ERR";
