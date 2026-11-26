use std::time::Duration;

pub const TAB_HEADER_ROWS: usize = 2;
pub const MAX_WEB_LOG_LINES: usize = 512;
pub const MAX_SERVER_TAB_LINES: usize = 2000;
pub const MAX_PROMPT_INPUT_CHARS: usize = 4096;
pub const TUI_EXIT_ACK_TIMEOUT_MS: u64 = 500;
pub const TUI_TASK_JOIN_TIMEOUT_MS: u64 = 300;
pub const COPY_FEEDBACK_TTL: Duration = Duration::from_secs(4);
