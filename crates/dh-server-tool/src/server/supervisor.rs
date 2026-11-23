use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dh_gadget_injector::{
    spawn_and_inject_with_cancel, ActiveInjectionSession, EventCallback, InjectLaunchRequest,
    InjectorError, InjectorEvent, LogLevel, OepStubInjector, PatchMod,
};
use dh_win32::{find_udp_listener_pid, is_process_alive, terminate_process, UdpLookupResult};
use tokio_util::sync::CancellationToken;

use crate::cli::{run_reconfigure_match_cli, LaunchPlan};
use crate::colors;
use crate::config::AppConfig;
use crate::format::format_seconds;
use crate::messages::ja::{app as app_msg, server as server_msg};
use crate::mods::{build_injection_plan, PatchScript, ResolvedPatchScript, DEFAULT_PATCH_SCRIPTS};
use crate::server::patch_logger::{
    append_patch_runtime_log, reset_patch_runtime_log, PatchLogLevel,
    PATCH_RUNTIME_LOG_RELATIVE_PATH,
};
use crate::terminal::{wait_for_post_match_decision, PostMatchAction, Terminal};

pub const UE_PROJECT_NAME: &str = "DreadHunger";
pub const LEGACY_GADGET_DLL_FILENAME: &str = "frida.dll";
pub const STARTUP_RETRY_DELAY_MS: u64 = 5_000;
pub const UDP_MISSING_STREAK_LIMIT: u32 = 3;
pub const INJECTION_INIT_TIMEOUT_SECS: u64 = 30;
pub const DETACH_EXIT_GRACE_MS: u64 = 500;

fn map_injector_log_level(level: LogLevel) -> PatchLogLevel {
    match level {
        LogLevel::Info => PatchLogLevel::Info,
        LogLevel::Warn => PatchLogLevel::Warn,
        LogLevel::Error => PatchLogLevel::Error,
    }
}

fn format_mod_loaded_line(idx: usize, total: usize, label: &str, file_name: &str) -> String {
    let blue_target: String = format!("{}{label} ({file_name})", colors::LIGHT_BLUE);
    format!(
        "{}   {}",
        colors::LIGHT_GREEN,
        server_msg::injected_patch_done(idx, total, &blue_target),
    )
}

fn format_spawned_process_line(pid: u32) -> String {
    format!(
        "{} * {} {}{pid}{})",
        colors::LIGHT_YELLOW,
        server_msg::SPAWNED_PROCESS_PREFIX,
        colors::LIGHT_WHITE,
        colors::LIGHT_YELLOW,
    )
}

fn alternate_port(ports: [u16; 2], current: u16) -> u16 {
    if current == ports[0] {
        ports[1]
    } else {
        ports[0]
    }
}

/// Resolves a relative executable path against `base_dir` without producing a Windows
/// `\\?\` verbatim prefix (which UE4's launch-directory parser can mishandle).
fn resolve_clean_path(path: &Path, base_dir: &Path) -> PathBuf {
    let joined: PathBuf = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    };
    let mut normalized: PathBuf = PathBuf::new();
    for comp in joined.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

async fn sleep_abortable(duration: Duration, cancel: &CancellationToken) -> bool {
    if cancel.is_cancelled() {
        return false;
    }
    tokio::select! {
        () = cancel.cancelled() => false,
        () = tokio::time::sleep(duration) => true,
    }
}

fn print_round_config_summary(
    terminal: &Terminal,
    plan: &LaunchPlan,
    current_port: u16,
    title: &str,
) {
    let default_mod_names: String = DEFAULT_PATCH_SCRIPTS
        .iter()
        .map(|patch: &PatchScript| patch.label)
        .collect::<Vec<&str>>()
        .join(", ");
    let selected_mod_names: String = if plan.selected_option_labels.is_empty() {
        app_msg::NONE_LABEL.to_string()
    } else {
        plan.selected_option_labels.join(", ")
    };

    terminal.println(&format!("{} {title}", colors::LIGHT_CYAN));
    terminal.println(&format!(
        "{} * {} {}{} ({})",
        colors::LIGHT_WHITE,
        app_msg::SUMMARY_MAP_LABEL,
        colors::LIGHT_GREEN,
        plan.map_label,
        plan.map_id
    ));
    terminal.println(&format!(
        "{} * {} {}{} <-> {} {}{}",
        colors::LIGHT_WHITE,
        app_msg::SUMMARY_SERVER_PORT_LABEL,
        colors::LIGHT_GREEN,
        plan.server_ports[0],
        plan.server_ports[1],
        colors::LIGHT_BLACK,
        server_msg::round_summary_current_port(current_port)
    ));
    terminal.println(&format!(
        "{} * {} {}{default_mod_names}",
        colors::LIGHT_WHITE,
        app_msg::SUMMARY_DEFAULT_MODS_LABEL,
        colors::CYAN,
    ));
    terminal.println(&format!(
        "{} * {} {}{selected_mod_names}",
        colors::LIGHT_WHITE,
        app_msg::SUMMARY_SELECTED_MODS_LABEL,
        colors::CYAN,
    ));
    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        server_msg::ROUND_SUMMARY_FOOTER
    ));
}

async fn spawn_and_inject_round(
    terminal: &Terminal,
    config: &AppConfig,
    ue_map_argument: &str,
    port: u16,
    round: u64,
    patches: &[ResolvedPatchScript],
    cancel: &CancellationToken,
) -> Result<ActiveInjectionSession, InjectorError> {
    if cancel.is_cancelled() {
        return Err(InjectorError::Cancelled);
    }

    // Overwrite ./patch_runtime.log at the start of every match round.
    reset_patch_runtime_log(port, patches);

    terminal.println(&format!(
        "{} * {}",
        colors::LIGHT_CYAN,
        server_msg::spawning_suspended(&config.shipping_executable, port)
    ));

    let total: usize = patches.len();
    let mut mod_meta: HashMap<String, (usize, String)> = HashMap::with_capacity(total);
    let mut injector_mods: Vec<PatchMod> = Vec::with_capacity(total);

    for (idx, patch) in patches.iter().enumerate() {
        mod_meta.insert(patch.file_name.clone(), (idx + 1, patch.label.clone()));
        injector_mods.push(PatchMod {
            id: patch.id.clone(),
            file_name: patch.file_name.clone(),
            label: patch.label.clone(),
            source: patch.source.clone(),
        });
    }

    let meta_arc: Arc<HashMap<String, (usize, String)>> = Arc::new(mod_meta);
    let loaded_counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
    let term_for_cb: Terminal = terminal.clone();

    let on_event: EventCallback = Arc::new(move |event: InjectorEvent| match event {
        InjectorEvent::ModLoaded { file_name } => {
            let seq: usize = loaded_counter.fetch_add(1, Ordering::Relaxed) + 1;
            let (idx, label): (usize, &str) = meta_arc
                .get(&file_name)
                .map_or((seq, file_name.as_str()), |(i, l): &(usize, String)| {
                    (*i, l.as_str())
                });
            term_for_cb.println(&format_mod_loaded_line(idx, total, label, &file_name));
        }
        InjectorEvent::Log {
            level,
            file_name,
            message,
        } => {
            // Runtime JS patch logs go exclusively to ./patch_runtime.log (never Tab 2).
            append_patch_runtime_log(map_injector_log_level(level), &file_name, &message);
        }
        InjectorEvent::InitError { file_name, message } => {
            append_patch_runtime_log(PatchLogLevel::Error, &file_name, &message);
            term_for_cb.println(&format!(
                "{}   {}",
                colors::RED,
                server_msg::patch_init_error(&file_name, &message)
            ));
        }
        InjectorEvent::InitOk | InjectorEvent::ChannelClosed => {}
    });

    let cwd: Option<PathBuf> = std::env::current_dir().ok();
    let exe_path: PathBuf = match cwd.as_deref() {
        Some(base) => resolve_clean_path(Path::new(&config.shipping_executable), base),
        None => PathBuf::from(&config.shipping_executable),
    };
    let args: Vec<String> = vec![
        UE_PROJECT_NAME.to_string(),
        ue_map_argument.to_string(),
        "-log".to_string(),
        format!("-port={port}"),
    ];

    let mut request: InjectLaunchRequest = InjectLaunchRequest::new(exe_path, args, injector_mods);
    request.cwd = cwd;
    request.patch_log_path = PathBuf::from(PATCH_RUNTIME_LOG_RELATIVE_PATH);
    request.round = round;
    request.init_timeout = Duration::from_secs(INJECTION_INIT_TIMEOUT_SECS);
    request.on_event = Some(on_event);

    let injector: OepStubInjector = OepStubInjector;
    let session: ActiveInjectionSession =
        spawn_and_inject_with_cancel(&injector, request, cancel.cancelled()).await?;

    terminal.println(&format_spawned_process_line(session.pid()));
    terminal.println(&format!(
        "{} * {}",
        colors::LIGHT_GREEN,
        server_msg::all_patches_injected_resumed(total)
    ));

    Ok(session)
}

/// Runs the alternating-port game server lifecycle loop:
/// spawn+OEP-inject -> 30s `INIT_OK` handshake -> UDP+process watchdog ->
/// post-match countdown (`R` reconfigure / `Enter` immediate restart) -> alternate port.
pub async fn run_server_supervisor(
    terminal: &Terminal,
    initial_plan: LaunchPlan,
    config: &AppConfig,
    cancel: &CancellationToken,
) {
    let mut active_plan: LaunchPlan = initial_plan;
    let mut patches: Vec<ResolvedPatchScript> = build_injection_plan(
        &active_plan.selected_option_numbers,
        active_plan.console_patch_port,
    );

    let mut current_port: u16 = active_plan.server_ports[0];
    let mut round: u32 = 0;
    let mut pending_retry_notice: Option<String> = None;

    while !cancel.is_cancelled() {
        round = round.saturating_add(1);

        if let Some(notice) = pending_retry_notice.take() {
            terminal.reset_server_tab_round();
            terminal.println(&format!(
                "{} * {}",
                colors::LIGHT_YELLOW,
                server_msg::retry_notice(&notice)
            ));
            print_round_config_summary(
                terminal,
                &active_plan,
                current_port,
                server_msg::ROUND_SUMMARY_RETRY_TITLE,
            );
        }

        terminal.println_empty();
        terminal.println(&format!(
            "{} {}",
            colors::LIGHT_CYAN,
            server_msg::round_launch_header(round, current_port)
        ));
        terminal.println(&format!(
            "{} * {} {}{} ({})  {}{} {}{current_port}",
            colors::LIGHT_WHITE,
            server_msg::ROUND_MAP_LABEL,
            colors::LIGHT_GREEN,
            active_plan.map_label,
            active_plan.map_id,
            colors::LIGHT_WHITE,
            server_msg::ROUND_PORT_LABEL,
            colors::LIGHT_GREEN,
        ));

        let session: ActiveInjectionSession = match spawn_and_inject_round(
            terminal,
            config,
            &active_plan.ue_map_argument,
            current_port,
            u64::from(round),
            &patches,
            cancel,
        )
        .await
        {
            Ok(session) => session,
            Err(InjectorError::Cancelled) => break,
            Err(err) => {
                if cancel.is_cancelled() {
                    break;
                }
                let msg: String = err.to_string();
                terminal.println(&format!(
                    "{} * {}",
                    colors::RED,
                    server_msg::startup_inject_error(&msg)
                ));
                terminal.println(&format!(
                    "{} * {}",
                    colors::RED,
                    server_msg::startup_retry_after(&format_seconds(STARTUP_RETRY_DELAY_MS))
                ));
                pending_retry_notice = Some(msg);
                current_port = alternate_port(active_plan.server_ports, current_port);
                let _ =
                    sleep_abortable(Duration::from_millis(STARTUP_RETRY_DELAY_MS), cancel).await;
                continue;
            }
        };

        let shipping_pid: u32 = session.pid();

        if !cancel.is_cancelled() {
            terminal.println(&format!(
                "{} * {}",
                colors::LIGHT_CYAN,
                server_msg::monitoring_server(&format_seconds(config.check_interval_ms))
            ));
            terminal.println(&format!(
                "{} * {}",
                colors::LIGHT_BLACK,
                server_msg::EXIT_HINT
            ));

            // Watchdog loop: lazily tracks UDP listener bind so early startup never false-triggers.
            let mut udp_ever_observed: bool = false;
            let mut udp_missing_streak: u32 = 0;

            while !cancel.is_cancelled() {
                if !sleep_abortable(Duration::from_millis(config.check_interval_ms), cancel).await {
                    break;
                }

                if !is_process_alive(shipping_pid) || !session.is_alive() {
                    break;
                }

                let udp: UdpLookupResult = find_udp_listener_pid(current_port, Some(shipping_pid));
                if udp.available {
                    if udp.pid == Some(shipping_pid) {
                        if !udp_ever_observed {
                            udp_ever_observed = true;
                            terminal.println(&format!(
                                "{} * {}",
                                colors::LIGHT_GREEN,
                                server_msg::udp_ready_confirmed(shipping_pid, current_port)
                            ));
                        }
                        udp_missing_streak = 0;
                    } else if udp_ever_observed {
                        udp_missing_streak += 1;
                        if udp_missing_streak >= UDP_MISSING_STREAK_LIMIT {
                            terminal.println(&format!(
                                "{} * {}",
                                colors::LIGHT_YELLOW,
                                server_msg::udp_stopped_restarting(current_port)
                            ));
                            let _ = terminate_process(shipping_pid);
                            break;
                        }
                    }
                }
            }
        }

        // Cleanup: grace window if exiting normally, then full session teardown & JobObject drop.
        if !cancel.is_cancelled() && is_process_alive(shipping_pid) {
            let _ = sleep_abortable(Duration::from_millis(DETACH_EXIT_GRACE_MS), cancel).await;
        }
        session.cleanup().await;
        if is_process_alive(shipping_pid) {
            let _ = terminate_process(shipping_pid);
        }

        if cancel.is_cancelled() {
            break;
        }

        let prev_port: u16 = current_port;
        current_port = alternate_port(active_plan.server_ports, current_port);

        terminal.println_empty();
        terminal.println(&format!(
            "{} * {} {}{prev_port} -> {}{current_port}",
            colors::LIGHT_YELLOW,
            server_msg::SERVER_EXIT_DETECTED_PREFIX,
            colors::LIGHT_WHITE,
            colors::LIGHT_GREEN,
        ));

        let action: PostMatchAction =
            wait_for_post_match_decision(terminal, config.restart_wait_seconds, cancel).await;

        match action {
            PostMatchAction::Abort => break,
            PostMatchAction::Reconfigure => {
                terminal.reset_server_tab_round();
                let Some(updated_plan): Option<LaunchPlan> =
                    run_reconfigure_match_cli(terminal, &active_plan, cancel).await
                else {
                    break;
                };
                if cancel.is_cancelled() {
                    break;
                }
                active_plan = updated_plan;
                patches = build_injection_plan(
                    &active_plan.selected_option_numbers,
                    active_plan.console_patch_port,
                );
                terminal.println_empty();
                print_round_config_summary(
                    terminal,
                    &active_plan,
                    current_port,
                    server_msg::ROUND_SUMMARY_UPDATED_TITLE,
                );
            }
            PostMatchAction::Restart => {
                terminal.reset_server_tab_round();
                print_round_config_summary(
                    terminal,
                    &active_plan,
                    current_port,
                    server_msg::ROUND_SUMMARY_CONTINUED_TITLE,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternates_server_ports_symmetrically() {
        let ports = [7895, 7897];
        assert_eq!(alternate_port(ports, 7895), 7897);
        assert_eq!(alternate_port(ports, 7897), 7895);
    }

    #[test]
    fn resolves_relative_shipping_executable_without_verbatim_prefix() {
        let base = Path::new(r"C:\Game\WindowsServer");
        let rel = Path::new("./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe");
        let resolved = resolve_clean_path(rel, base);
        let s = resolved.to_string_lossy();
        assert!(!s.starts_with(r"\\?\"));
        assert!(s.ends_with(r"DreadHunger\Binaries\Win64\DreadHungerServer-Win64-Shipping.exe"));
    }

    #[test]
    fn formats_mod_loaded_line_with_light_green_prefix_and_light_blue_name() {
        let line = format_mod_loaded_line(1, 13, "Webコンソール連携", "コンソール連携.js");
        assert_eq!(
            line,
            "\x1b[92m   [1/13] 注入完了: \x1b[94mWebコンソール連携 (コンソール連携.js)"
        );
    }

    #[test]
    fn formats_spawned_process_line_with_light_yellow_and_white_pid() {
        let line = format_spawned_process_line(12345);
        assert_eq!(
            line,
            "\x1b[93m * プロセス生成完了 (PID: \x1b[97m12345\x1b[93m)"
        );
    }
}
