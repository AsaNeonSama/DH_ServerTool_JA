pub mod display;
pub mod post_match;
pub mod tab_renderer;
pub mod tabs;

use std::io::{self, BufRead, IsTerminal, Stdin, StdinLock, Stdout, Write};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::colors;
use crate::languages;
use crate::structs::languages::TerminalMsgCode;
pub use crate::structs::terminal::Terminal;
use crate::structs::terminal::{ModeState, PromptResult, TerminalInner};
use crate::web_console::logger::WebLogSink;
use crate::web_console::WebConsoleDisplayInfo;

pub use display::{
    compute_max_scroll_offset, count_visual_rows, display_width, measure_prompt_cursor,
    pad_display_width, select_visible_lines, strip_ansi, truncate_line_to_display_width,
    CursorMeasurement, VisibleLinesResult,
};
pub use post_match::{wait_for_post_match_decision, PostMatchAction};
pub use tab_renderer::{
    HitRegion, TerminalTabId, MAX_SERVER_TAB_LINES, MAX_WEB_LOG_LINES, TAB_HEADER_ROWS,
};
pub use tabs::{install_panic_hook, restore_terminal_emergency, PostMatchKey, PromptError};

impl Terminal {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TerminalInner {
                stdin_is_tty: io::stdin().is_terminal(),
                stdout_is_tty: io::stdout().is_terminal(),
                mode: Mutex::new(ModeState {
                    tx: None,
                    task: None,
                    pre_tab_web_logs: Vec::new(),
                }),
            }),
        }
    }

    /// Initializes the Windows console (UTF-8 CP, title, VT processing) and clears
    /// the screen if stdout is an interactive TTY.
    pub fn setup(&self, title: &str) {
        install_panic_hook();
        dh_win32::console::init_console(title);
        if self.inner.stdout_is_tty {
            let mut stdout: Stdout = io::stdout();
            let _ = write!(stdout, "\x1b[2J\x1b[H\x1b]0;{title}\x07");
            let _ = stdout.flush();
        }
    }

    #[must_use]
    pub fn is_interactive_tty(&self) -> bool {
        self.inner.stdin_is_tty && self.inner.stdout_is_tty
    }

    #[must_use]
    pub fn is_tabbed_mode_active(&self) -> bool {
        self.inner
            .mode
            .lock()
            .is_ok_and(|guard: MutexGuard<'_, ModeState>| guard.tx.is_some())
    }

    /// Builds a `WebLogSink` closure wired directly to Tab 1's 512-line log buffer
    /// (buffering lines emitted before entering tabbed mode so startup logs are never lost).
    #[must_use]
    pub fn web_log_sink(&self) -> WebLogSink {
        let term: Self = self.clone();
        Arc::new(move |line: String| {
            term.append_web_log(line);
        })
    }

    pub fn append_web_log(&self, text: impl Into<String>) {
        let text: String = text.into();
        if let Ok(mut guard) = self.inner.mode.lock() {
            if let Some(tx) = &guard.tx {
                let _ = tx.send(tabs::TuiCommand::AppendWebLog(text));
            } else {
                for line in text.split('\n').map(|s: &str| s.trim_end_matches('\r')) {
                    guard.pre_tab_web_logs.push(line.to_string());
                }
                if guard.pre_tab_web_logs.len() > MAX_WEB_LOG_LINES {
                    let excess: usize = guard.pre_tab_web_logs.len() - MAX_WEB_LOG_LINES;
                    guard.pre_tab_web_logs.drain(0..excess);
                }
            }
        }
    }

    pub fn println(&self, text: &str) {
        if let Ok(guard) = self.inner.mode.lock() {
            if let Some(tx) = &guard.tx {
                let _ = tx.send(tabs::TuiCommand::AppendServerLog(text.to_string()));
                return;
            }
        }
        let mut stdout: Stdout = io::stdout();
        let _ = writeln!(stdout, "{text}{}", colors::RESET);
        let _ = stdout.flush();
    }

    pub fn println_empty(&self) {
        self.println("");
    }

    pub fn reset_server_tab_round(&self) {
        if let Ok(guard) = self.inner.mode.lock() {
            if let Some(tx) = &guard.tx {
                let _ = tx.send(tabs::TuiCommand::ResetServerRound);
            }
        }
    }

    pub fn set_server_status_line(&self, line: Option<String>) {
        if let Ok(guard) = self.inner.mode.lock() {
            if let Some(tx) = &guard.tx {
                let _ = tx.send(tabs::TuiCommand::SetStatusLine(line));
            }
        }
    }

    pub fn set_post_match_channel(&self, ch: Option<mpsc::UnboundedSender<PostMatchKey>>) {
        if let Ok(guard) = self.inner.mode.lock() {
            if let Some(tx) = &guard.tx {
                let _ = tx.send(tabs::TuiCommand::SetPostMatchChannel(ch));
            }
        }
    }

    pub fn switch_tab(&self, tab: TerminalTabId) {
        if let Ok(guard) = self.inner.mode.lock() {
            if let Some(tx) = &guard.tx {
                let _ = tx.send(tabs::TuiCommand::SwitchTab(tab));
            }
        }
    }

    /// Enters Alternate Screen dual-tab TUI mode when running on an interactive TTY.
    pub fn enter_tabbed_mode(
        &self,
        info: WebConsoleDisplayInfo,
        initial_tab: TerminalTabId,
        shutdown_token: CancellationToken,
    ) -> io::Result<bool> {
        if !self.is_interactive_tty() {
            return Ok(false);
        }

        let mut guard: MutexGuard<'_, ModeState> = self
            .inner
            .mode
            .lock()
            .map_err(|_| io::Error::other("terminal state mutex poisoned"))?;
        if guard.tx.is_some() {
            return Ok(true);
        }

        let initial_web_logs: Vec<String> = std::mem::take(&mut guard.pre_tab_web_logs);
        let (tx, task): (mpsc::UnboundedSender<tabs::TuiCommand>, JoinHandle<()>) =
            tabs::spawn_tui_task(info, initial_tab, initial_web_logs, shutdown_token)?;
        guard.tx = Some(tx);
        guard.task = Some(task);
        Ok(true)
    }

    /// Cleanly leaves Alternate Screen dual-tab TUI mode and restores normal terminal state.
    pub async fn exit_tabbed_mode(&self) {
        let (tx, task): (
            Option<mpsc::UnboundedSender<tabs::TuiCommand>>,
            Option<JoinHandle<()>>,
        ) = {
            let Ok(mut guard): Result<
                MutexGuard<'_, ModeState>,
                PoisonError<MutexGuard<'_, ModeState>>,
            > = self.inner.mode.lock() else {
                restore_terminal_emergency();
                return;
            };
            (guard.tx.take(), guard.task.take())
        };

        if let Some(tx) = tx {
            let (ack_tx, ack_rx): (oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel();
            if tx.send(tabs::TuiCommand::Exit(ack_tx)).is_ok() {
                let _ = tokio::time::timeout(Duration::from_millis(500), ack_rx).await;
            }
        }
        if let Some(task) = task {
            let _ = tokio::time::timeout(Duration::from_millis(300), task).await;
        }
        restore_terminal_emergency();
    }

    /// Synchronous best-effort exit (e.g. from signal or drop paths).
    pub fn exit_tabbed_mode_sync(&self) {
        if let Ok(mut guard) = self.inner.mode.lock() {
            if let Some(tx) = guard.tx.take() {
                let (ack_tx, _ack_rx): (oneshot::Sender<()>, oneshot::Receiver<()>) =
                    oneshot::channel();
                let _ = tx.send(tabs::TuiCommand::Exit(ack_tx));
            }
            if let Some(task) = guard.task.take() {
                task.abort();
            }
        }
        restore_terminal_emergency();
    }

    /// Prompts the user for a line of input (in Tab 2 when tabbed mode is active,
    /// or via standard stdin/stdout otherwise), honoring `cancel`.
    pub async fn ask(
        &self,
        prompt_text: &str,
        cancel: Option<&CancellationToken>,
    ) -> Result<String, PromptError> {
        if cancel.is_some_and(CancellationToken::is_cancelled) {
            return Err(PromptError::Aborted);
        }

        let maybe_tx: Option<mpsc::UnboundedSender<tabs::TuiCommand>> = self
            .inner
            .mode
            .lock()
            .ok()
            .and_then(|guard: MutexGuard<'_, ModeState>| guard.tx.clone());

        if let Some(tx) = maybe_tx {
            let (reply_tx, reply_rx): (
                oneshot::Sender<PromptResult>,
                oneshot::Receiver<PromptResult>,
            ) = oneshot::channel();
            tx.send(tabs::TuiCommand::StartPrompt {
                prompt_text: prompt_text.to_string(),
                reply: reply_tx,
            })
            .map_err(|_| PromptError::Aborted)?;

            return match cancel {
                Some(token) => {
                    tokio::select! {
                        () = token.cancelled() => {
                            let _ = tx.send(tabs::TuiCommand::CancelPrompt);
                            Err(PromptError::Aborted)
                        }
                        res = reply_rx => res.unwrap_or(Err(PromptError::Aborted)),
                    }
                }
                None => reply_rx.await.unwrap_or(Err(PromptError::Aborted)),
            };
        }

        // Plain stdin/stdout prompt (Stage 1 before TUI or non-TTY fallback)
        {
            let mut stdout: Stdout = io::stdout();
            let _ = write!(stdout, "{prompt_text}{}", colors::RESET);
            let _ = stdout.flush();
        }

        let read_task: JoinHandle<Result<String, PromptError>> =
            tokio::task::spawn_blocking(|| -> Result<String, PromptError> {
                let stdin: Stdin = io::stdin();
                let mut line: String = String::new();
                let mut lock: StdinLock<'_> = stdin.lock();
                match lock.read_line(&mut line) {
                    Ok(0) => Err(PromptError::Aborted),
                    Ok(_) => Ok(line.trim().to_string()),
                    Err(err) => Err(PromptError::Io(err.to_string())),
                }
            });

        match cancel {
            Some(token) => {
                tokio::select! {
                    () = token.cancelled() => Err(PromptError::Aborted),
                    res = read_task => res.unwrap_or(Err(PromptError::Aborted)),
                }
            }
            None => read_task.await.unwrap_or(Err(PromptError::Aborted)),
        }
    }

    pub async fn wait_for_exit_key(&self) {
        if !self.inner.stdin_is_tty {
            return;
        }
        let prompt: String = format!(
            "\n{} * {} ",
            colors::LIGHT_YELLOW,
            languages::terminal_text(TerminalMsgCode::PressEnterToExit)
        );
        let _ = self.ask(&prompt, None).await;
    }
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new()
    }
}
