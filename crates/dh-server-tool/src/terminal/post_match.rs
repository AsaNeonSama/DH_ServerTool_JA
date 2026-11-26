use std::io::{self, Stdout, Write};
use std::time::Duration;

use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio::time::Interval;
use tokio_util::sync::CancellationToken;

use crate::colors;
use crate::languages;
use crate::structs::languages::TerminalMsgCode;
pub use crate::structs::terminal::PostMatchAction;
use crate::structs::terminal::RawModeGuard;
use crate::terminal::tabs::PostMatchKey;
use crate::terminal::Terminal;

#[must_use]
pub fn build_countdown_text(remaining: u64) -> String {
    format!(
        "{} * {} {}{remaining}{} {}{}R{}{} {}Enter{}{}",
        colors::LIGHT_YELLOW,
        languages::terminal_text(TerminalMsgCode::PostMatchCountdownPrefix),
        colors::LIGHT_GREEN,
        colors::LIGHT_YELLOW,
        languages::terminal_text(TerminalMsgCode::PostMatchCountdownMid),
        colors::LIGHT_CYAN,
        colors::LIGHT_YELLOW,
        languages::terminal_text(TerminalMsgCode::PostMatchCountdownReconfigure),
        colors::LIGHT_CYAN,
        colors::LIGHT_YELLOW,
        languages::terminal_text(TerminalMsgCode::PostMatchCountdownRestart),
    )
}

async fn wait_for_post_match_decision_tabbed(
    terminal: &Terminal,
    total_seconds: u64,
    cancel: &CancellationToken,
) -> PostMatchAction {
    let (key_tx, mut key_rx): (
        mpsc::UnboundedSender<PostMatchKey>,
        mpsc::UnboundedReceiver<PostMatchKey>,
    ) = mpsc::unbounded_channel::<PostMatchKey>();
    terminal.set_post_match_channel(Some(key_tx));

    let mut remaining: u64 = total_seconds;
    terminal.set_server_status_line(Some(build_countdown_text(remaining)));

    let mut interval: Interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // Consume the immediate first tick so the countdown waits a full second before decrementing.
    interval.tick().await;

    let action: PostMatchAction = loop {
        tokio::select! {
            () = cancel.cancelled() => break PostMatchAction::Abort,
            maybe_key = key_rx.recv() => {
                match maybe_key {
                    Some(PostMatchKey::Reconfigure) => break PostMatchAction::Reconfigure,
                    Some(PostMatchKey::Restart) => break PostMatchAction::Restart,
                    None => break PostMatchAction::Abort,
                }
            }
            _ = interval.tick() => {
                remaining = remaining.saturating_sub(1);
                if remaining == 0 {
                    break PostMatchAction::Restart;
                }
                terminal.set_server_status_line(Some(build_countdown_text(remaining)));
            }
        }
    };

    terminal.set_post_match_channel(None);
    terminal.set_server_status_line(None);
    action
}

impl RawModeGuard {
    fn enter() -> Option<Self> {
        enable_raw_mode().ok().map(|()| Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout: Stdout = io::stdout();
        let _ = stdout.write_all(b"\n");
        let _ = stdout.flush();
    }
}

fn render_plain_countdown(remaining: u64) {
    let mut stdout: Stdout = io::stdout();
    let _ = write!(
        stdout,
        "\r\x1b[K{}{}",
        build_countdown_text(remaining),
        colors::RESET
    );
    let _ = stdout.flush();
}

async fn wait_for_post_match_decision_plain_tty(
    total_seconds: u64,
    cancel: &CancellationToken,
) -> PostMatchAction {
    let _raw_guard: Option<RawModeGuard> = RawModeGuard::enter();
    let mut remaining: u64 = total_seconds;

    render_plain_countdown(remaining);
    let mut events: EventStream = EventStream::new();
    let mut events_ended: bool = false;
    let mut interval: Interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;

    loop {
        tokio::select! {
            () = cancel.cancelled() => return PostMatchAction::Abort,
            maybe_ev = events.next(), if !events_ended => {
                match maybe_ev {
                    Some(Ok(Event::Key(key))) => {
                        if key.kind != KeyEventKind::Press {
                            continue;
                        }
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && matches!(key.code, KeyCode::Char('c' | 'C'))
                        {
                            cancel.cancel();
                            return PostMatchAction::Abort;
                        }
                        match key.code {
                            KeyCode::Char(ch)
                                if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                                    && ch.eq_ignore_ascii_case(&'r') =>
                            {
                                return PostMatchAction::Reconfigure;
                            }
                            KeyCode::Enter => return PostMatchAction::Restart,
                            _ => {}
                        }
                    }
                    Some(Ok(_) | Err(_)) => {}
                    None => {
                        events_ended = true;
                    }
                }
            }
            _ = interval.tick() => {
                remaining = remaining.saturating_sub(1);
                if remaining == 0 {
                    return PostMatchAction::Restart;
                }
                render_plain_countdown(remaining);
            }
        }
    }
}

/// Waits `seconds` after a match ends, allowing `R` to reconfigure or `Enter` to restart immediately.
pub async fn wait_for_post_match_decision(
    terminal: &Terminal,
    seconds: u64,
    cancel: &CancellationToken,
) -> PostMatchAction {
    if cancel.is_cancelled() {
        return PostMatchAction::Abort;
    }

    let total_seconds: u64 = seconds.max(1);

    if terminal.is_tabbed_mode_active() {
        return wait_for_post_match_decision_tabbed(terminal, total_seconds, cancel).await;
    }

    if !terminal.is_interactive_tty() {
        return tokio::select! {
            () = cancel.cancelled() => PostMatchAction::Abort,
            () = tokio::time::sleep(Duration::from_secs(total_seconds)) => PostMatchAction::Restart,
        };
    }

    wait_for_post_match_decision_plain_tty(total_seconds, cancel).await
}
