use std::io::{self, Stdout, Write};
use std::panic::PanicHookInfo;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Once;
use std::time::Duration;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, size as terminal_size, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::colors;
use crate::consts::terminal::COPY_FEEDBACK_TTL;
use crate::languages;
use crate::structs::languages::{TerminalMsg, TerminalMsgCode, WebLogEvent};
use crate::structs::terminal::{ActivePrompt, PanicHookFn, TabModeGuard, TuiState};
pub use crate::structs::terminal::{PostMatchKey, PromptError, TuiCommand};
use crate::terminal::tab_renderer::{
    render_frame_buffer, CopyFeedbackState, FrameRenderInput, HitRegion, RenderedFrame,
    TerminalTabId, MAX_SERVER_TAB_LINES, MAX_WEB_LOG_LINES,
};
use crate::web_console::logger::render_web_log_event;
use crate::web_console::WebConsoleDisplayInfo;

static TUI_ACTIVE: AtomicBool = AtomicBool::new(false);
static HAS_SAVED_INPUT_MODE: AtomicBool = AtomicBool::new(false);
static SAVED_INPUT_MODE: AtomicU32 = AtomicU32::new(0);
static PANIC_HOOK_ONCE: Once = Once::new();

impl std::fmt::Display for PromptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aborted => write!(f, "Prompt aborted"),
            Self::Io(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for PromptError {}

/// Installs a process-wide panic hook that unconditionally exits Alternate Screen,
/// disables mouse capture/raw mode, and restores the original Win32 console mode
/// before printing the panic message.
pub fn install_panic_hook() {
    PANIC_HOOK_ONCE.call_once(|| {
        let previous: PanicHookFn = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info: &PanicHookInfo<'_>| {
            restore_terminal_emergency();
            previous(info);
        }));
    });
}

/// Idempotent emergency terminal restoration usable from `Drop`, signal handlers, or panics.
pub fn restore_terminal_emergency() {
    if !TUI_ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }

    let mut stdout: Stdout = io::stdout();
    let _ = stdout.write_all(b"\x1b[?2026l\x1b[?1006l\x1b[?1000l\x1b[?25h\x1b[?1049l\x1b[0m");
    let _ = stdout.flush();
    let _ = execute!(stdout, DisableMouseCapture, LeaveAlternateScreen);
    let _ = disable_raw_mode();

    if HAS_SAVED_INPUT_MODE.swap(false, Ordering::SeqCst) {
        let mode: u32 = SAVED_INPUT_MODE.load(Ordering::SeqCst);
        let _ = dh_win32::console::set_input_mode(mode);
    }
}

#[must_use]
pub fn is_tui_active() -> bool {
    TUI_ACTIVE.load(Ordering::SeqCst)
}

impl TabModeGuard {
    fn enter() -> io::Result<Self> {
        install_panic_hook();

        if let Some(mode) = dh_win32::console::get_input_mode() {
            SAVED_INPUT_MODE.store(mode, Ordering::SeqCst);
            HAS_SAVED_INPUT_MODE.store(true, Ordering::SeqCst);
        }

        enable_raw_mode()?;
        TUI_ACTIVE.store(true, Ordering::SeqCst);

        let mut stdout: Stdout = io::stdout();
        // Enter Alternate Screen + EnableMouseCapture (disables Quick Edit in crossterm Windows
        // while keeping native Win32 MOUSE_EVENT / KEY_EVENT delivery intact for EventStream).
        if let Err(err) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            restore_terminal_emergency();
            return Err(err);
        }
        let _ = stdout.write_all(b"\x1b[3J\x1b[2J\x1b[H");
        let _ = stdout.flush();

        Ok(Self)
    }
}

impl Drop for TabModeGuard {
    fn drop(&mut self) {
        restore_terminal_emergency();
    }
}

impl TuiState {
    fn new(
        info: WebConsoleDisplayInfo,
        initial_tab: TerminalTabId,
        initial_web_logs: Vec<String>,
    ) -> Self {
        let mut web_log_lines: Vec<String> = initial_web_logs;
        if web_log_lines.len() > MAX_WEB_LOG_LINES {
            let excess: usize = web_log_lines.len() - MAX_WEB_LOG_LINES;
            web_log_lines.drain(0..excess);
        }
        Self {
            active_tab: initial_tab,
            web_console_info: info,
            server_lines: Vec::new(),
            server_scroll_offset: 0,
            web_log_lines,
            web_log_scroll_offset: 0,
            status_line: None,
            active_prompt: None,
            post_match_tx: None,
            copy_feedback: None,
            copy_feedback_deadline: None,
            tab1_hit: None,
            tab2_hit: None,
            public_btn_hit: None,
            local_btn_hit: None,
        }
    }

    fn current_terminal_size() -> (usize, usize) {
        match terminal_size() {
            Ok((cols, rows)) => (usize::from(cols).max(40), usize::from(rows).max(10)),
            Err(_) => (120, 30),
        }
    }

    fn render(&mut self) {
        let (term_cols, term_rows): (usize, usize) = Self::current_terminal_size();
        let prompt_text: Option<&str> = self
            .active_prompt
            .as_ref()
            .map(|p: &ActivePrompt| p.prompt_text.as_str());
        let prompt_input: &str = self
            .active_prompt
            .as_ref()
            .map_or("", |p: &ActivePrompt| p.input_buffer.as_str());

        let rendered: RenderedFrame = render_frame_buffer(&FrameRenderInput {
            term_cols,
            term_rows,
            active_tab: self.active_tab,
            web_console_info: Some(&self.web_console_info),
            copy_feedback: self.copy_feedback.as_ref(),
            web_log_lines: &self.web_log_lines,
            web_log_scroll_offset: self.web_log_scroll_offset,
            server_lines: &self.server_lines,
            server_scroll_offset: self.server_scroll_offset,
            status_line: self.status_line.as_deref(),
            prompt_text,
            prompt_input,
        });

        self.tab1_hit = Some(rendered.tab1_hit);
        self.tab2_hit = Some(rendered.tab2_hit);
        self.public_btn_hit = rendered.public_btn_hit;
        self.local_btn_hit = rendered.local_btn_hit;
        self.server_scroll_offset = rendered.clamped_server_offset;
        self.web_log_scroll_offset = rendered.clamped_web_offset;

        let mut stdout: Stdout = io::stdout();
        let _ = stdout.write_all(rendered.ansi_frame.as_bytes());
        let _ = stdout.flush();
    }

    fn append_web_log(&mut self, text: &str) {
        let split_lines: Vec<&str> = text
            .split('\n')
            .map(|s: &str| s.trim_end_matches('\r'))
            .collect();
        let added_count: usize = split_lines.len();
        for line in split_lines {
            self.web_log_lines.push(line.to_string());
        }
        let mut trimmed_count: usize = 0;
        if self.web_log_lines.len() > MAX_WEB_LOG_LINES {
            trimmed_count = self.web_log_lines.len() - MAX_WEB_LOG_LINES;
            self.web_log_lines.drain(0..trimmed_count);
        }
        if self.web_log_scroll_offset > 0 {
            self.web_log_scroll_offset = self
                .web_log_scroll_offset
                .saturating_add(added_count)
                .saturating_sub(trimmed_count);
        }
        if self.active_tab == TerminalTabId::Web {
            self.render();
        }
    }

    fn append_server_log(&mut self, text: &str) {
        for line in text.split('\n').map(|s: &str| s.trim_end_matches('\r')) {
            self.server_lines.push(line.to_string());
        }
        if self.server_lines.len() > MAX_SERVER_TAB_LINES {
            let excess: usize = self.server_lines.len() - MAX_SERVER_TAB_LINES;
            self.server_lines.drain(0..excess);
        }
        if self.active_tab == TerminalTabId::Server {
            self.render();
        }
    }

    fn reset_server_round(&mut self) {
        self.server_lines.clear();
        self.server_scroll_offset = 0;
        self.status_line = None;
        let mut stdout: Stdout = io::stdout();
        let _ = stdout.write_all(b"\x1b[3J");
        let _ = stdout.flush();
        if self.active_tab == TerminalTabId::Server {
            self.render();
        }
    }

    fn switch_tab(&mut self, tab: TerminalTabId) {
        if self.active_tab == tab {
            return;
        }
        self.active_tab = tab;
        self.render();
    }

    fn adjust_scroll(&mut self, delta: isize) {
        match self.active_tab {
            TerminalTabId::Web => {
                self.web_log_scroll_offset =
                    self.web_log_scroll_offset.saturating_add_signed(delta);
            }
            TerminalTabId::Server => {
                self.server_scroll_offset = self.server_scroll_offset.saturating_add_signed(delta);
            }
        }
        self.render();
    }

    fn jump_scroll_top(&mut self) {
        match self.active_tab {
            TerminalTabId::Web => self.web_log_scroll_offset = self.web_log_lines.len(),
            TerminalTabId::Server => self.server_scroll_offset = self.server_lines.len(),
        }
        self.render();
    }

    fn jump_scroll_bottom(&mut self) {
        match self.active_tab {
            TerminalTabId::Web => self.web_log_scroll_offset = 0,
            TerminalTabId::Server => self.server_scroll_offset = 0,
        }
        self.render();
    }

    fn trigger_copy_url(&mut self, is_public: bool) {
        let target_url: String = if is_public {
            self.web_console_info.public_url.clone()
        } else {
            self.web_console_info.local_url.clone()
        };

        let ok: bool = dh_win32::console::set_clipboard_text(&target_url).is_ok();
        let feedback_text: String = if ok {
            languages::render_terminal(&TerminalMsg::CopySuccessFeedback { is_public })
        } else {
            languages::terminal_text(TerminalMsgCode::CopyFailureFeedback).to_string()
        };

        self.copy_feedback = Some(CopyFeedbackState {
            text: feedback_text,
            is_error: !ok,
        });
        self.copy_feedback_deadline = Some(Instant::now() + COPY_FEEDBACK_TTL);

        let log_line: String = render_web_log_event(&WebLogEvent::UrlCopied {
            is_public,
            success: ok,
        });
        self.append_web_log(&log_line);
    }

    fn abort_active_prompt(&mut self) {
        if let Some(pending) = self.active_prompt.take() {
            let _ = pending.reply.send(Err(PromptError::Aborted));
        }
    }

    fn handle_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::ScrollUp => {
                self.adjust_scroll(3);
            }
            MouseEventKind::ScrollDown => {
                self.adjust_scroll(-3);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let col: usize = usize::from(ev.column) + 1;
                let row: usize = usize::from(ev.row) + 1;
                if self
                    .tab1_hit
                    .is_some_and(|r: HitRegion| r.contains(col, row))
                {
                    self.switch_tab(TerminalTabId::Web);
                    return;
                }
                if self
                    .tab2_hit
                    .is_some_and(|r: HitRegion| r.contains(col, row))
                {
                    self.switch_tab(TerminalTabId::Server);
                    return;
                }
                if self.active_tab == TerminalTabId::Web {
                    if self
                        .public_btn_hit
                        .is_some_and(|r: HitRegion| r.contains(col, row))
                    {
                        self.trigger_copy_url(true);
                        return;
                    }
                    if self
                        .local_btn_hit
                        .is_some_and(|r: HitRegion| r.contains(col, row))
                    {
                        self.trigger_copy_url(false);
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_key(&mut self, ev: KeyEvent, shutdown_token: &CancellationToken) {
        // Windows crossterm delivers Press, Repeat, and Release; ignore Release to avoid duplicates.
        if ev.kind == KeyEventKind::Release {
            return;
        }

        // Ctrl+C always triggers graceful shutdown from any tab.
        if ev.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(ev.code, KeyCode::Char('c' | 'C'))
        {
            if ev.kind == KeyEventKind::Press {
                self.abort_active_prompt();
                shutdown_token.cancel();
            }
            return;
        }

        match ev.code {
            KeyCode::Left => {
                self.switch_tab(TerminalTabId::Web);
                return;
            }
            KeyCode::Right => {
                self.switch_tab(TerminalTabId::Server);
                return;
            }
            KeyCode::Up => {
                self.adjust_scroll(1);
                return;
            }
            KeyCode::Down => {
                self.adjust_scroll(-1);
                return;
            }
            KeyCode::PageUp => {
                self.adjust_scroll(5);
                return;
            }
            KeyCode::PageDown => {
                self.adjust_scroll(-5);
                return;
            }
            KeyCode::Home => {
                self.jump_scroll_top();
                return;
            }
            KeyCode::End => {
                self.jump_scroll_bottom();
                return;
            }
            _ => {}
        }

        if self.active_tab == TerminalTabId::Web {
            if ev.kind == KeyEventKind::Press
                && !ev
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            {
                if let KeyCode::Char(ch) = ev.code {
                    match ch.to_ascii_lowercase() {
                        'p' => self.trigger_copy_url(true),
                        'l' => self.trigger_copy_url(false),
                        _ => {}
                    }
                }
            }
            return;
        }

        // Tab 2: post-match countdown key handler takes precedence over prompts.
        if let Some(tx) = &self.post_match_tx {
            if ev.kind == KeyEventKind::Press {
                match ev.code {
                    KeyCode::Char(ch)
                        if !ev
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                            && ch.eq_ignore_ascii_case(&'r') =>
                    {
                        let _ = tx.send(PostMatchKey::Reconfigure);
                        return;
                    }
                    KeyCode::Enter => {
                        let _ = tx.send(PostMatchKey::Restart);
                        return;
                    }
                    _ => {}
                }
            }
        }

        // Tab 2: active prompt input editing.
        if self.active_prompt.is_none() {
            return;
        }

        match ev.code {
            KeyCode::Enter => {
                if ev.kind != KeyEventKind::Press {
                    return;
                }
                if let Some(prompt) = self.active_prompt.take() {
                    let committed: String = prompt.input_buffer;
                    self.server_scroll_offset = 0;
                    self.append_server_log(&format!(
                        "{}{}{committed}",
                        prompt.prompt_text,
                        colors::RESET,
                    ));
                    let _ = prompt.reply.send(Ok(committed.trim().to_string()));
                }
            }
            KeyCode::Backspace => {
                if let Some(prompt) = &mut self.active_prompt {
                    if prompt.input_buffer.pop().is_some() {
                        self.server_scroll_offset = 0;
                        self.render();
                    }
                }
            }
            KeyCode::Char('u' | 'U') if ev.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(prompt) = &mut self.active_prompt {
                    if !prompt.input_buffer.is_empty() {
                        prompt.input_buffer.clear();
                        self.server_scroll_offset = 0;
                        self.render();
                    }
                }
            }
            KeyCode::Char(ch)
                if !ev
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && u32::from(ch) >= 0x20 =>
            {
                if let Some(prompt) = &mut self.active_prompt {
                    prompt.input_buffer.push(ch);
                    self.server_scroll_offset = 0;
                    self.render();
                }
            }
            _ => {}
        }
    }
}

async fn sleep_until_optional(deadline: Option<Instant>) {
    match deadline {
        Some(target) => tokio::time::sleep_until(target).await,
        None => std::future::pending::<()>().await,
    }
}

/// Spawns the dedicated TUI render & input task over an unbounded command channel.
pub fn spawn_tui_task(
    info: WebConsoleDisplayInfo,
    initial_tab: TerminalTabId,
    initial_web_logs: Vec<String>,
    shutdown_token: CancellationToken,
) -> io::Result<(mpsc::UnboundedSender<TuiCommand>, JoinHandle<()>)> {
    let guard: TabModeGuard = TabModeGuard::enter()?;
    let (tx, mut rx): (
        mpsc::UnboundedSender<TuiCommand>,
        mpsc::UnboundedReceiver<TuiCommand>,
    ) = mpsc::unbounded_channel::<TuiCommand>();

    let handle: JoinHandle<()> = tokio::spawn(async move {
        let mode_guard: TabModeGuard = guard;
        let mut state: TuiState = TuiState::new(info, initial_tab, initial_web_logs);
        state.render();

        let mut events: EventStream = EventStream::new();

        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(TuiCommand::AppendWebLog(line)) => state.append_web_log(&line),
                        Some(TuiCommand::AppendServerLog(line)) => state.append_server_log(&line),
                        Some(TuiCommand::ResetServerRound) => state.reset_server_round(),
                        Some(TuiCommand::SetStatusLine(line)) => {
                            state.status_line = line;
                            if state.active_tab == TerminalTabId::Server {
                                state.render();
                            }
                        }
                        Some(TuiCommand::StartPrompt { prompt_text, reply }) => {
                            state.abort_active_prompt();
                            state.active_prompt = Some(ActivePrompt {
                                prompt_text,
                                input_buffer: String::new(),
                                reply,
                            });
                            state.server_scroll_offset = 0;
                            state.render();
                        }
                        Some(TuiCommand::CancelPrompt) => {
                            state.abort_active_prompt();
                            if state.active_tab == TerminalTabId::Server {
                                state.render();
                            }
                        }
                        Some(TuiCommand::SetPostMatchChannel(ch)) => {
                            state.post_match_tx = ch;
                        }
                        Some(TuiCommand::SwitchTab(tab)) => state.switch_tab(tab),
                        Some(TuiCommand::Exit(ack)) => {
                            state.abort_active_prompt();
                            state.post_match_tx = None;
                            state.status_line = None;
                            drop(events);
                            drop(mode_guard);
                            let _ = ack.send(());
                            return;
                        }
                        None => {
                            state.abort_active_prompt();
                            return;
                        }
                    }
                }
                maybe_ev = events.next() => {
                    match maybe_ev {
                        Some(Ok(Event::Key(key_ev))) => {
                            state.handle_key(key_ev, &shutdown_token);
                        }
                        Some(Ok(Event::Mouse(mouse_ev))) => {
                            state.handle_mouse(mouse_ev);
                        }
                        Some(Ok(Event::Resize(_, _))) => {
                            state.render();
                        }
                        Some(Ok(_)) => {}
                        Some(Err(_)) | None => {
                            // Input stream ended; keep rendering commands until explicit Exit.
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    }
                }
                () = sleep_until_optional(state.copy_feedback_deadline) => {
                    state.copy_feedback = None;
                    state.copy_feedback_deadline = None;
                    if state.active_tab == TerminalTabId::Web {
                        state.render();
                    }
                }
            }
        }
    });

    Ok((tx, handle))
}
