use std::panic::PanicHookInfo;
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::structs::web_console::WebConsoleDisplayInfo;

/// Result of slicing a log buffer to fit the visible viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleLinesResult {
    pub visible_lines: Vec<String>,
    pub clamped_offset: usize,
}

/// Cursor measurement after rendering a prompt + user input line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorMeasurement {
    /// Total visual rows reserved for rendering the prompt and cursor.
    pub visual_rows: usize,
    /// Zero-based row offset from the prompt's start row where the cursor lands.
    pub row_offset: usize,
    /// One-based terminal column where the cursor lands.
    pub cursor_col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostMatchAction {
    Reconfigure,
    Restart,
    Abort,
}

pub(crate) struct RawModeGuard;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTabId {
    Web,
    Server,
}

/// One-based inclusive hit-test rectangle on a single terminal row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitRegion {
    pub row: usize,
    pub start_col: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyFeedbackState {
    pub text: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabBarLayout {
    pub row1: String,
    pub row2: String,
    pub tab1_hit: HitRegion,
    pub tab2_hit: HitRegion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebTabLayout {
    pub lines: Vec<String>,
    pub public_btn_hit: Option<HitRegion>,
    pub local_btn_hit: Option<HitRegion>,
}

pub struct FrameRenderInput<'a> {
    pub term_cols: usize,
    pub term_rows: usize,
    pub active_tab: TerminalTabId,
    pub web_console_info: Option<&'a WebConsoleDisplayInfo>,
    pub copy_feedback: Option<&'a CopyFeedbackState>,
    pub web_log_lines: &'a [String],
    pub web_log_scroll_offset: usize,
    pub server_lines: &'a [String],
    pub server_scroll_offset: usize,
    pub status_line: Option<&'a str>,
    pub prompt_text: Option<&'a str>,
    pub prompt_input: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedFrame {
    pub ansi_frame: String,
    pub tab1_hit: HitRegion,
    pub tab2_hit: HitRegion,
    pub public_btn_hit: Option<HitRegion>,
    pub local_btn_hit: Option<HitRegion>,
    pub clamped_server_offset: usize,
    pub clamped_web_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptError {
    Aborted,
    Io(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostMatchKey {
    Reconfigure,
    Restart,
}

pub enum TuiCommand {
    AppendWebLog(String),
    AppendServerLog(String),
    ResetServerRound,
    SetStatusLine(Option<String>),
    StartPrompt {
        prompt_text: String,
        reply: oneshot::Sender<Result<String, PromptError>>,
    },
    CancelPrompt,
    SetPostMatchChannel(Option<mpsc::UnboundedSender<PostMatchKey>>),
    SwitchTab(TerminalTabId),
    Exit(oneshot::Sender<()>),
}

pub(crate) type PanicHookFn = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static>;
pub(crate) type PromptResult = Result<String, PromptError>;

/// RAII guard that owns Alternate Screen + Raw Mode + Mouse Capture and guarantees
/// restoration on normal exit, cancellation, or unwind/panic.
pub(crate) struct TabModeGuard;

pub(crate) struct ActivePrompt {
    pub(crate) prompt_text: String,
    pub(crate) input_buffer: String,
    pub(crate) reply: oneshot::Sender<Result<String, PromptError>>,
}

pub(crate) struct TuiState {
    pub(crate) active_tab: TerminalTabId,
    pub(crate) web_console_info: WebConsoleDisplayInfo,
    pub(crate) server_lines: Vec<String>,
    pub(crate) server_scroll_offset: usize,
    pub(crate) web_log_lines: Vec<String>,
    pub(crate) web_log_scroll_offset: usize,
    pub(crate) status_line: Option<String>,
    pub(crate) active_prompt: Option<ActivePrompt>,
    pub(crate) post_match_tx: Option<mpsc::UnboundedSender<PostMatchKey>>,
    pub(crate) copy_feedback: Option<CopyFeedbackState>,
    pub(crate) copy_feedback_deadline: Option<Instant>,
    pub(crate) tab1_hit: Option<HitRegion>,
    pub(crate) tab2_hit: Option<HitRegion>,
    pub(crate) public_btn_hit: Option<HitRegion>,
    pub(crate) local_btn_hit: Option<HitRegion>,
}

pub(crate) struct ModeState {
    pub(crate) tx: Option<mpsc::UnboundedSender<TuiCommand>>,
    pub(crate) task: Option<JoinHandle<()>>,
    pub(crate) pre_tab_web_logs: Vec<String>,
}

pub(crate) struct TerminalInner {
    pub(crate) stdin_is_tty: bool,
    pub(crate) stdout_is_tty: bool,
    pub(crate) mode: Mutex<ModeState>,
}

/// Thread-safe terminal controller supporting interactive dual-tab Alternate Screen TUI
/// and automatic plain stdin/stdout fallback when non-TTY.
#[derive(Clone)]
pub struct Terminal {
    pub(crate) inner: Arc<TerminalInner>,
}
