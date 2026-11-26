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
pub use crate::consts::server::{
    DETACH_EXIT_GRACE_MS, INJECTION_INIT_TIMEOUT_SECS, LEGACY_GADGET_DLL_FILENAME,
    STARTUP_RETRY_DELAY_MS, UDP_MISSING_STREAK_LIMIT, UE_PROJECT_NAME,
};
use crate::format::format_seconds;
use crate::languages;
use crate::mods::{
    prepare_injection_plan, ModGroup, PatchScript, ResolvedPatchScript, DEFAULT_PATCH_SCRIPTS,
};
use crate::server::patch_logger::{
    append_patch_runtime_log, reset_patch_runtime_log, PatchLogLevel,
    PATCH_RUNTIME_LOG_RELATIVE_PATH,
};
use crate::structs::languages::{AppMsgCode, ServerMsg, ServerMsgCode};
use crate::terminal::{wait_for_post_match_decision, PostMatchAction, Terminal};

fn map_injector_log_level(level: LogLevel) -> PatchLogLevel {
    match level {
        LogLevel::Info => PatchLogLevel::Info,
        LogLevel::Warn => PatchLogLevel::Warn,
        LogLevel::Error => PatchLogLevel::Error,
    }
}

#[must_use]
pub fn format_mod_loaded_line_for_group(
    idx: usize,
    total: usize,
    label: &str,
    file_name: &str,
    group: ModGroup,
) -> String {
    let target_color: &str = if group == ModGroup::ContentMod {
        colors::LIGHT_MAGENTA
    } else {
        colors::LIGHT_BLUE
    };
    let styled_target: String = format!("{target_color}{label} ({file_name})");
    format!(
        "{}   {}",
        colors::LIGHT_GREEN,
        languages::render_server(&ServerMsg::InjectedPatchDone {
            index: idx,
            total,
            label: &styled_target,
        }),
    )
}

#[must_use]
pub fn format_mod_loaded_line(idx: usize, total: usize, label: &str, file_name: &str) -> String {
    format_mod_loaded_line_for_group(idx, total, label, file_name, ModGroup::Default)
}

#[must_use]
pub fn format_spawned_process_line(pid: u32) -> String {
    format!(
        "{} * {} {}{pid}{})",
        colors::LIGHT_YELLOW,
        languages::server_text(ServerMsgCode::SpawnedProcessPrefix),
        colors::LIGHT_WHITE,
        colors::LIGHT_YELLOW,
    )
}

#[must_use]
pub fn alternate_port(ports: [u16; 2], current: u16) -> u16 {
    if current == ports[0] {
        ports[1]
    } else {
        ports[0]
    }
}

#[must_use]
pub fn next_server_port(ports: [u16; 2], current: u16, switch_port_on_restart: bool) -> u16 {
    if switch_port_on_restart {
        alternate_port(ports, current)
    } else {
        current
    }
}

/// Resolves a relative executable path against `base_dir` without producing a Windows
/// `\\?\` verbatim prefix (which UE4's launch-directory parser can mishandle).
#[must_use]
pub fn resolve_clean_path(path: &Path, base_dir: &Path) -> PathBuf {
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
                let _: bool = normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    let normalized_str: std::borrow::Cow<'_, str> = normalized.to_string_lossy();
    if let Some(stripped) = normalized_str.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        normalized
    }
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
    title_code: ServerMsgCode,
) {
    let default_mod_names: String = DEFAULT_PATCH_SCRIPTS
        .iter()
        .map(|patch: &PatchScript| languages::mod_label(patch.label_code))
        .collect::<Vec<&str>>()
        .join(", ");
    let content_mod_names: String = if plan.selected_content_option_labels.is_empty() {
        languages::app_text(AppMsgCode::NoneLabel).to_string()
    } else {
        plan.selected_content_option_labels.join(", ")
    };
    let selected_mod_names: String = if plan.selected_option_labels.is_empty() {
        languages::app_text(AppMsgCode::NoneLabel).to_string()
    } else {
        plan.selected_option_labels.join(", ")
    };

    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        languages::server_text(title_code)
    ));
    terminal.println(&format!(
        "{} * {} {}{} ({})",
        colors::LIGHT_WHITE,
        languages::app_text(AppMsgCode::SummaryMapLabel),
        colors::LIGHT_GREEN,
        languages::cli_text(plan.map_label_code),
        plan.map_id
    ));
    if plan.switch_port_on_restart {
        terminal.println(&format!(
            "{} * {} {}{} <-> {} {}{}",
            colors::LIGHT_WHITE,
            languages::app_text(AppMsgCode::SummaryServerPortLabel),
            colors::LIGHT_GREEN,
            plan.server_ports[0],
            plan.server_ports[1],
            colors::LIGHT_BLACK,
            languages::render_server(&ServerMsg::RoundSummaryCurrentPort { port: current_port })
        ));
    } else {
        terminal.println(&format!(
            "{} * {} {}{} {}{}",
            colors::LIGHT_WHITE,
            languages::app_text(AppMsgCode::SummaryServerPortLabel),
            colors::LIGHT_GREEN,
            current_port,
            colors::LIGHT_BLACK,
            languages::server_text(ServerMsgCode::RoundSummaryFixedPortSuffix),
        ));
    }
    terminal.println(&format!(
        "{} * {} {}{default_mod_names}",
        colors::LIGHT_WHITE,
        languages::app_text(AppMsgCode::SummaryDefaultModsLabel),
        colors::CYAN,
    ));
    terminal.println(&format!(
        "{} * {} {}{content_mod_names}",
        colors::LIGHT_WHITE,
        languages::app_text(AppMsgCode::SummaryContentModsLabel),
        colors::LIGHT_MAGENTA,
    ));
    terminal.println(&format!(
        "{} * {} {}{selected_mod_names}",
        colors::LIGHT_WHITE,
        languages::app_text(AppMsgCode::SummarySelectedModsLabel),
        colors::CYAN,
    ));
    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        languages::server_text(ServerMsgCode::RoundSummaryFooter)
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
        languages::render_server(&ServerMsg::SpawningSuspended {
            executable: &config.shipping_executable,
            port,
        })
    ));

    let total: usize = patches.len();
    let mut mod_meta: HashMap<String, (usize, String, ModGroup)> = HashMap::with_capacity(total);
    let mut injector_mods: Vec<PatchMod> = Vec::with_capacity(total);

    for (idx, patch) in patches.iter().enumerate() {
        mod_meta.insert(
            patch.file_name.clone(),
            (idx + 1, patch.label.clone(), patch.group),
        );
        injector_mods.push(PatchMod {
            id: patch.id.clone(),
            file_name: patch.file_name.clone(),
            label: patch.label.clone(),
            source: patch.source.clone(),
        });
    }

    let meta_arc: Arc<HashMap<String, (usize, String, ModGroup)>> = Arc::new(mod_meta);
    let loaded_counter: Arc<AtomicUsize> = Arc::new(AtomicUsize::new(0));
    let term_for_cb: Terminal = terminal.clone();

    let on_event: EventCallback = Arc::new(move |event: InjectorEvent| match event {
        InjectorEvent::ModLoaded { file_name } => {
            let seq: usize = loaded_counter
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1);
            let (idx, label, group): (usize, &str, ModGroup) = meta_arc.get(&file_name).map_or(
                (seq, file_name.as_str(), ModGroup::Default),
                |(i, l, g): &(usize, String, ModGroup)| (*i, l.as_str(), *g),
            );
            term_for_cb.println(&format_mod_loaded_line_for_group(
                idx, total, label, &file_name, group,
            ));
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
                languages::render_server(&ServerMsg::PatchInitError {
                    file_name: &file_name,
                    error_text: &message,
                })
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
        languages::render_server(&ServerMsg::AllPatchesInjectedResumed { count: total })
    ));

    Ok(session)
}

fn prepare_round_patches(
    terminal: &Terminal,
    plan: &LaunchPlan,
) -> Option<Vec<ResolvedPatchScript>> {
    match prepare_injection_plan(
        &plan.selected_content_option_numbers,
        &plan.selected_option_numbers,
        plan.console_patch_port,
        None,
    ) {
        Ok(list) => Some(list),
        Err(err) => {
            terminal.println(&format!(
                "{} * {}",
                colors::RED,
                languages::render_server(&ServerMsg::ContentModAssetPrepareError {
                    error_text: &err,
                })
            ));
            None
        }
    }
}

/// Runs the game server lifecycle loop:
/// spawn+OEP-inject -> 30s `INIT_OK` handshake -> UDP+process watchdog ->
/// post-match countdown (`R` reconfigure / `Enter` immediate restart) ->
/// alternate port (when `switch_port_on_restart` is enabled) or keep the same fixed port.
pub async fn run_server_supervisor(
    terminal: &Terminal,
    initial_plan: LaunchPlan,
    config: &AppConfig,
    cancel: &CancellationToken,
) {
    let mut active_plan: LaunchPlan = initial_plan;
    let Some(mut patches): Option<Vec<ResolvedPatchScript>> =
        prepare_round_patches(terminal, &active_plan)
    else {
        return;
    };

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
                languages::render_server(&ServerMsg::RetryNotice {
                    error_text: &notice,
                    switch_port_on_restart: active_plan.switch_port_on_restart,
                })
            ));
            print_round_config_summary(
                terminal,
                &active_plan,
                current_port,
                ServerMsgCode::RoundSummaryRetryTitle,
            );
        }

        terminal.println_empty();
        terminal.println(&format!(
            "{} {}",
            colors::LIGHT_CYAN,
            languages::render_server(&ServerMsg::RoundLaunchHeader {
                round,
                port: current_port,
            })
        ));
        terminal.println(&format!(
            "{} * {} {}{} ({})  {}{} {}{current_port}",
            colors::LIGHT_WHITE,
            languages::server_text(ServerMsgCode::RoundMapLabel),
            colors::LIGHT_GREEN,
            languages::cli_text(active_plan.map_label_code),
            active_plan.map_id,
            colors::LIGHT_WHITE,
            languages::server_text(ServerMsgCode::RoundPortLabel),
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
                let delay_text: String = format_seconds(STARTUP_RETRY_DELAY_MS);
                terminal.println(&format!(
                    "{} * {}",
                    colors::RED,
                    languages::render_server(&ServerMsg::StartupInjectError { error_text: &msg })
                ));
                terminal.println(&format!(
                    "{} * {}",
                    colors::RED,
                    languages::render_server(&ServerMsg::StartupRetryAfter {
                        delay_text: &delay_text,
                        switch_port_on_restart: active_plan.switch_port_on_restart,
                    })
                ));
                pending_retry_notice = Some(msg);
                current_port = next_server_port(
                    active_plan.server_ports,
                    current_port,
                    active_plan.switch_port_on_restart,
                );
                let _ =
                    sleep_abortable(Duration::from_millis(STARTUP_RETRY_DELAY_MS), cancel).await;
                continue;
            }
        };

        let shipping_pid: u32 = session.pid();

        if !cancel.is_cancelled() {
            let interval_text: String = format_seconds(config.check_interval_ms);
            terminal.println(&format!(
                "{} * {}",
                colors::LIGHT_CYAN,
                languages::render_server(&ServerMsg::MonitoringServer {
                    interval_text: &interval_text,
                    switch_port_on_restart: active_plan.switch_port_on_restart,
                })
            ));
            terminal.println(&format!(
                "{} * {}",
                colors::LIGHT_BLACK,
                languages::server_text(ServerMsgCode::ExitHint)
            ));

            // Watchdog loop: lazily tracks UDP listener bind so early startup never false-triggers.
            let mut udp_ever_observed: bool = false;
            let mut udp_missing_streak: u32 = 0;

            while !cancel.is_cancelled() {
                if !sleep_abortable(Duration::from_millis(config.check_interval_ms), cancel).await {
                    break;
                }

                if !session.is_alive() || !is_process_alive(shipping_pid) {
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
                                languages::render_server(&ServerMsg::UdpReadyConfirmed {
                                    pid: shipping_pid,
                                    port: current_port,
                                })
                            ));
                        }
                        udp_missing_streak = 0;
                    } else if udp_ever_observed {
                        udp_missing_streak = udp_missing_streak.saturating_add(1);
                        if udp_missing_streak >= UDP_MISSING_STREAK_LIMIT {
                            terminal.println(&format!(
                                "{} * {}",
                                colors::LIGHT_YELLOW,
                                languages::render_server(&ServerMsg::UdpStoppedRestarting {
                                    port: current_port,
                                })
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
        current_port = next_server_port(
            active_plan.server_ports,
            current_port,
            active_plan.switch_port_on_restart,
        );

        terminal.println_empty();
        if active_plan.switch_port_on_restart {
            terminal.println(&format!(
                "{} * {} {}{prev_port} -> {}{current_port}",
                colors::LIGHT_YELLOW,
                languages::server_text(ServerMsgCode::ServerExitDetectedPrefix),
                colors::LIGHT_WHITE,
                colors::LIGHT_GREEN,
            ));
        } else {
            terminal.println(&format!(
                "{} * {} {}{current_port} {}{}",
                colors::LIGHT_YELLOW,
                languages::server_text(ServerMsgCode::ServerExitDetectedPrefix),
                colors::LIGHT_GREEN,
                colors::LIGHT_BLACK,
                languages::server_text(ServerMsgCode::ServerExitFixedPortSuffix),
            ));
        }

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
                let Some(next_patches): Option<Vec<ResolvedPatchScript>> =
                    prepare_round_patches(terminal, &active_plan)
                else {
                    break;
                };
                patches = next_patches;
                terminal.println_empty();
                print_round_config_summary(
                    terminal,
                    &active_plan,
                    current_port,
                    ServerMsgCode::RoundSummaryUpdatedTitle,
                );
            }
            PostMatchAction::Restart => {
                terminal.reset_server_tab_round();
                print_round_config_summary(
                    terminal,
                    &active_plan,
                    current_port,
                    ServerMsgCode::RoundSummaryContinuedTitle,
                );
            }
        }
    }
}
