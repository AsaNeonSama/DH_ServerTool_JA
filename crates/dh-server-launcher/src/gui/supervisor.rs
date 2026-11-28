use std::path::{Path, PathBuf};
use std::sync::{Arc, MutexGuard};
use std::time::Duration;

use dh_gadget_injector::{
    spawn_and_inject_with_cancel, ActiveInjectionSession, DllEventCallback, DllInjectionEvent,
    EventCallback, InjectLaunchRequest, InjectorError, InjectorEvent, LogLevel, OepStubInjector,
    PatchMod,
};
use dh_win32::{find_udp_listener_pid, is_process_alive, terminate_process, UdpLookupResult};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::config::AppConfig;
use crate::consts::gui::{EVENT_SERVER_LOG, EVENT_SERVER_STATE};
use crate::consts::server::{
    DETACH_EXIT_GRACE_MS, INJECTION_INIT_TIMEOUT_SECS, SERVER_CHECK_INTERVAL_MS,
    STARTUP_RETRY_DELAY_MS, UDP_MISSING_STREAK_LIMIT, UE_PROJECT_NAME,
};
use crate::format::format_seconds;
use crate::gui::state::{build_patch_progress_list, idle_server_monitor_state};
use crate::languages;
use crate::mods::{
    prepare_injection_plan, prepare_server_dll_plan, ExtractedServerDll, ResolvedPatchScript,
};
use crate::server::patch_logger::{
    append_patch_runtime_log_at, start_patch_runtime_log, PatchLogLevel,
};
use crate::server::supervisor::{next_server_port, resolve_clean_path};
use crate::structs::gui::{
    GuiAppState, GuiRoundSpawnSpec, GuiSharedData, GuiSupervisorBootstrap, PatchProgressItemDto,
    PatchProgressStatus, ServerControlCommand, ServerLogEntryDto, ServerLogLevel,
    ServerMonitorStateDto, ServerPhase,
};
use crate::structs::languages::{GuiMsgCode, ServerMsg};
use crate::structs::server::LaunchPlan;

fn map_injector_log_level(level: LogLevel) -> PatchLogLevel {
    match level {
        LogLevel::Info => PatchLogLevel::Info,
        LogLevel::Warn => PatchLogLevel::Warn,
        LogLevel::Error => PatchLogLevel::Error,
    }
}

fn emit_server_log(app: &AppHandle, state: &GuiAppState, level: ServerLogLevel, message: String) {
    let entry: ServerLogEntryDto = state.push_server_log(level, message);
    let _: Result<(), tauri::Error> = app.emit(EVENT_SERVER_LOG, entry);
}

fn emit_server_state(app: &AppHandle, state: &GuiAppState) {
    let snapshot: ServerMonitorStateDto = state.lock_shared().server_status.clone();
    let _: Result<(), tauri::Error> = app.emit(EVENT_SERVER_STATE, snapshot);
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

async fn spawn_and_inject_gui_round(
    app: &AppHandle,
    state: &GuiAppState,
    spec: &GuiRoundSpawnSpec<'_>,
    cancel: &CancellationToken,
) -> Result<ActiveInjectionSession, InjectorError> {
    if cancel.is_cancelled() {
        return Err(InjectorError::Cancelled);
    }

    let log_path: PathBuf = start_patch_runtime_log(spec.port, spec.patches);
    let log_file_name: String = log_path
        .file_name()
        .map_or_default(|n: &std::ffi::OsStr| n.to_string_lossy().to_string());

    let total_scripts: usize = spec.patches.len();
    let total_dlls: usize = spec.dlls.len();
    let total_items: usize = total_scripts.saturating_add(total_dlls);

    {
        let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
        guard.server_status.current_patch_log_file = Some(log_file_name);
        guard.server_status.patch_progress = build_patch_progress_list(spec.patches);
        guard.server_status.injected_count = 0;
        guard.server_status.total_patch_count = total_items;
        guard.server_status.active_mod_count = total_items;
    }
    emit_server_state(app, state);

    emit_server_log(
        app,
        state,
        ServerLogLevel::Info,
        languages::render_server(&ServerMsg::SpawningSuspended {
            executable: &spec.config.shipping_executable,
            port: spec.port,
        }),
    );

    let mut injector_mods: Vec<PatchMod> = Vec::with_capacity(total_scripts);
    for patch in spec.patches {
        injector_mods.push(PatchMod {
            id: patch.id.clone(),
            file_name: patch.file_name.clone(),
            label: patch.label.clone(),
            source: patch.source.clone(),
        });
    }

    let app_for_cb: AppHandle = app.clone();
    let state_for_cb: GuiAppState = state.clone();
    let log_path_for_cb: Arc<PathBuf> = Arc::new(log_path.clone());

    let on_event: EventCallback = Arc::new(move |event: InjectorEvent| match event {
        InjectorEvent::ModLoaded { file_name } => {
            let mut matched_label: String = file_name.clone();
            let mut matched_index: usize = 0;
            {
                let mut guard: MutexGuard<'_, GuiSharedData> = state_for_cb.lock_shared();
                for item in &mut guard.server_status.patch_progress {
                    if item.file_name == file_name
                        && !matches!(item.status, PatchProgressStatus::Loaded)
                    {
                        item.status = PatchProgressStatus::Loaded;
                        matched_label.clone_from(&item.label);
                        matched_index = item.index;
                        break;
                    }
                }
                guard.server_status.injected_count = guard
                    .server_status
                    .injected_count
                    .saturating_add(1)
                    .min(total_items);
                if matched_index == 0 {
                    matched_index = guard
                        .server_status
                        .patch_progress
                        .iter()
                        .filter(|p: &&PatchProgressItemDto| {
                            matches!(p.status, PatchProgressStatus::Loaded)
                        })
                        .count();
                }
            }
            emit_server_state(&app_for_cb, &state_for_cb);
            let label_with_file: String = format!("{matched_label} ({file_name})");
            emit_server_log(
                &app_for_cb,
                &state_for_cb,
                ServerLogLevel::Success,
                languages::render_server(&ServerMsg::InjectedPatchDone {
                    index: matched_index,
                    total: total_scripts,
                    label: &label_with_file,
                }),
            );
        }
        InjectorEvent::Log {
            level,
            file_name,
            message,
        } => {
            append_patch_runtime_log_at(
                &log_path_for_cb,
                map_injector_log_level(level),
                &file_name,
                &message,
            );
        }
        InjectorEvent::InitError { file_name, message } => {
            append_patch_runtime_log_at(
                &log_path_for_cb,
                PatchLogLevel::Error,
                &file_name,
                &message,
            );
            {
                let mut guard: MutexGuard<'_, GuiSharedData> = state_for_cb.lock_shared();
                for item in &mut guard.server_status.patch_progress {
                    if item.file_name == file_name {
                        item.status = PatchProgressStatus::Error;
                        item.error = Some(message.clone());
                        break;
                    }
                }
            }
            emit_server_state(&app_for_cb, &state_for_cb);
            emit_server_log(
                &app_for_cb,
                &state_for_cb,
                ServerLogLevel::Error,
                languages::render_server(&ServerMsg::PatchInitError {
                    file_name: &file_name,
                    error_text: &message,
                }),
            );
        }
        InjectorEvent::InitOk | InjectorEvent::ChannelClosed => {}
    });

    let dll_meta: Arc<Vec<(String, String)>> = Arc::new(
        spec.dlls
            .iter()
            .map(|d: &ExtractedServerDll| (d.file_name.clone(), d.label.clone()))
            .collect(),
    );
    let app_for_dll: AppHandle = app.clone();
    let state_for_dll: GuiAppState = state.clone();
    let log_path_for_dll: Arc<PathBuf> = Arc::new(log_path.clone());
    let on_dll_event: DllEventCallback = Arc::new(move |event: DllInjectionEvent| match event {
        DllInjectionEvent::Loaded {
            index,
            total: dll_total,
            path,
            module_handle,
        } => {
            let fallback_name: String = path
                .file_name()
                .map_or_default(|n: &std::ffi::OsStr| n.to_string_lossy().to_string());
            let (file_name, label): (&str, &str) = dll_meta.get(index.saturating_sub(1)).map_or(
                (fallback_name.as_str(), fallback_name.as_str()),
                |(f, l)| (f.as_str(), l.as_str()),
            );
            {
                let mut guard: MutexGuard<'_, GuiSharedData> = state_for_dll.lock_shared();
                guard.server_status.injected_count = guard
                    .server_status
                    .injected_count
                    .saturating_add(1)
                    .min(total_items);
            }
            emit_server_state(&app_for_dll, &state_for_dll);
            let msg: String = languages::render_server(&ServerMsg::OepDllInjected {
                index,
                total: dll_total,
                label,
                file_name,
                module_handle,
            });
            append_patch_runtime_log_at(&log_path_for_dll, PatchLogLevel::Info, file_name, &msg);
            emit_server_log(&app_for_dll, &state_for_dll, ServerLogLevel::Success, msg);
        }
        DllInjectionEvent::Failed {
            index,
            total: dll_total,
            path,
        } => {
            let fallback_name: String = path
                .file_name()
                .map_or_default(|n: &std::ffi::OsStr| n.to_string_lossy().to_string());
            let (file_name, label): (&str, &str) = dll_meta.get(index.saturating_sub(1)).map_or(
                (fallback_name.as_str(), fallback_name.as_str()),
                |(f, l)| (f.as_str(), l.as_str()),
            );
            let msg: String = languages::render_server(&ServerMsg::OepDllInjectFailed {
                index,
                total: dll_total,
                label,
                file_name,
            });
            append_patch_runtime_log_at(&log_path_for_dll, PatchLogLevel::Error, file_name, &msg);
            emit_server_log(&app_for_dll, &state_for_dll, ServerLogLevel::Error, msg);
        }
    });

    let cwd: Option<PathBuf> = std::env::current_dir().ok();
    let exe_path: PathBuf = match cwd.as_deref() {
        Some(base) => resolve_clean_path(Path::new(&spec.config.shipping_executable), base),
        None => PathBuf::from(&spec.config.shipping_executable),
    };
    let args: Vec<String> = vec![
        UE_PROJECT_NAME.to_string(),
        spec.ue_map_argument.to_string(),
        "-log".to_string(),
        format!("-port={}", spec.port),
    ];

    let mut request: InjectLaunchRequest = InjectLaunchRequest::new(exe_path, args, injector_mods);
    request.cwd = cwd;
    request.dlls = spec
        .dlls
        .iter()
        .map(|d: &ExtractedServerDll| d.path.clone())
        .collect();
    request.fail_on_dll_load_error = !request.dlls.is_empty();
    request.patch_log_path = log_path;
    request.round = spec.round;
    request.init_timeout = Duration::from_secs(INJECTION_INIT_TIMEOUT_SECS);
    request.on_event = Some(on_event);
    if !request.dlls.is_empty() {
        request.on_dll_event = Some(on_dll_event);
    }

    let injector: OepStubInjector = OepStubInjector;
    let session: ActiveInjectionSession =
        spawn_and_inject_with_cancel(&injector, request, cancel.cancelled()).await?;

    emit_server_log(
        app,
        state,
        ServerLogLevel::Success,
        languages::render_server(&ServerMsg::AllPatchesInjectedResumed { count: total_items }),
    );

    Ok(session)
}

/// Runs the GUI-controlled dedicated server supervisor loop with real-time Tauri state & log events.
pub async fn run_gui_server_supervisor(
    app: AppHandle,
    state: GuiAppState,
    bootstrap: GuiSupervisorBootstrap,
    cancel: CancellationToken,
    mut cmd_rx: mpsc::UnboundedReceiver<ServerControlCommand>,
) {
    let mut active_plan: LaunchPlan = bootstrap.initial_plan;
    let mut patches: Vec<ResolvedPatchScript> = bootstrap.initial_patches;
    let mut dlls: Vec<ExtractedServerDll> = bootstrap.initial_dlls;
    let config: AppConfig = bootstrap.config;
    let mut current_port: u16 = active_plan.server_ports[0];
    let mut round: u32 = 0;

    while !cancel.is_cancelled() {
        round = round.saturating_add(1);
        let map_label: String = languages::server_text(active_plan.map_label_code).to_string();

        {
            let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
            guard.server_status.phase = if round == 1 {
                ServerPhase::Launching
            } else {
                ServerPhase::Restarting
            };
            guard.server_status.pid = None;
            guard.server_status.active_port = Some(current_port);
            guard.server_status.next_port = None;
            guard.server_status.switch_port_on_restart = active_plan.switch_port_on_restart;
            guard.server_status.server_ports = active_plan.server_ports;
            guard.server_status.udp_confirmed = false;
            guard.server_status.round = round;
            guard.server_status.map_id = Some(active_plan.map_id.to_string());
            guard.server_status.map_label = Some(map_label.clone());
            guard.server_status.countdown_seconds = None;
            guard.server_status.last_error = None;
        }
        emit_server_state(&app, &state);
        emit_server_log(
            &app,
            &state,
            ServerLogLevel::Info,
            languages::render_server(&ServerMsg::RoundLaunchHeader {
                round,
                port: current_port,
            }),
        );

        let spawn_spec: GuiRoundSpawnSpec<'_> = GuiRoundSpawnSpec {
            config: &config,
            ue_map_argument: &active_plan.ue_map_argument,
            port: current_port,
            round: u64::from(round),
            patches: &patches,
            dlls: &dlls,
        };
        let session: ActiveInjectionSession =
            match spawn_and_inject_gui_round(&app, &state, &spawn_spec, &cancel).await {
                Ok(session) => session,
                Err(InjectorError::Cancelled) => break,
                Err(err) => {
                    if cancel.is_cancelled() {
                        break;
                    }
                    let msg: String = err.to_string();
                    {
                        let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
                        guard.server_status.last_error = Some(msg.clone());
                    }
                    emit_server_state(&app, &state);
                    emit_server_log(
                        &app,
                        &state,
                        ServerLogLevel::Error,
                        languages::render_server(&ServerMsg::StartupInjectError {
                            error_text: &msg,
                        }),
                    );

                    let watchdog_on: bool = state.lock_shared().ui_draft.auto_restart_watchdog;
                    if !watchdog_on {
                        emit_server_log(
                            &app,
                            &state,
                            ServerLogLevel::Warn,
                            languages::gui_text(GuiMsgCode::WatchdogOffStopped).to_string(),
                        );
                        break;
                    }

                    let delay_text: String = format_seconds(STARTUP_RETRY_DELAY_MS);
                    emit_server_log(
                        &app,
                        &state,
                        ServerLogLevel::Warn,
                        languages::render_server(&ServerMsg::StartupRetryAfter {
                            delay_text: &delay_text,
                            switch_port_on_restart: active_plan.switch_port_on_restart,
                        }),
                    );
                    current_port = next_server_port(
                        active_plan.server_ports,
                        current_port,
                        active_plan.switch_port_on_restart,
                    );
                    let _ = sleep_abortable(Duration::from_millis(STARTUP_RETRY_DELAY_MS), &cancel)
                        .await;
                    continue;
                }
            };

        let shipping_pid: u32 = session.pid();
        {
            let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
            guard.server_status.phase = ServerPhase::Running;
            guard.server_status.pid = Some(shipping_pid);
            guard.server_status.active_port = Some(current_port);
        }
        emit_server_state(&app, &state);

        let interval_text: String = format_seconds(SERVER_CHECK_INTERVAL_MS);
        emit_server_log(
            &app,
            &state,
            ServerLogLevel::Info,
            languages::render_server(&ServerMsg::MonitoringServer {
                interval_text: &interval_text,
                switch_port_on_restart: active_plan.switch_port_on_restart,
            }),
        );

        let mut udp_ever_observed: bool = false;
        let mut udp_missing_streak: u32 = 0;
        let mut immediate_restart: bool = false;

        while !cancel.is_cancelled() {
            tokio::select! {
                () = cancel.cancelled() => {
                    break;
                }
                maybe_cmd = cmd_rx.recv() => {
                    if let Some(cmd) = maybe_cmd {
                        match cmd {
                            ServerControlCommand::RestartNow => {
                                emit_server_log(
                                    &app,
                                    &state,
                                    ServerLogLevel::Warn,
                                    languages::gui_text(GuiMsgCode::ImmediateRestartRequested).to_string(),
                                );
                                immediate_restart = true;
                                let _ = terminate_process(shipping_pid);
                                break;
                            }
                            ServerControlCommand::UpdateNextPlan(new_plan) => {
                                let next_prepared: Result<
                                    (Vec<ResolvedPatchScript>, Vec<ExtractedServerDll>),
                                    String,
                                > = prepare_injection_plan(
                                    &new_plan.selected_content_option_numbers,
                                    &new_plan.selected_option_numbers,
                                    new_plan.console_patch_port,
                                    None,
                                )
                                .and_then(|next_patches: Vec<ResolvedPatchScript>| {
                                    let next_dlls: Vec<ExtractedServerDll> =
                                        prepare_server_dll_plan(
                                            &new_plan.selected_content_option_numbers,
                                            None,
                                        )?;
                                    Ok((next_patches, next_dlls))
                                });
                                match next_prepared {
                                    Ok((next_patches, next_dlls)) => {
                                        active_plan = new_plan;
                                        patches = next_patches;
                                        dlls = next_dlls;
                                        emit_server_log(
                                            &app,
                                            &state,
                                            ServerLogLevel::Info,
                                            languages::gui_text(GuiMsgCode::NextRoundPlanUpdated).to_string(),
                                        );
                                    }
                                    Err(err) => {
                                        emit_server_log(
                                            &app,
                                            &state,
                                            ServerLogLevel::Error,
                                            languages::render_server(&ServerMsg::ContentModAssetPrepareError {
                                                error_text: &err,
                                            }),
                                        );
                                    }
                                }
                            }
                            ServerControlCommand::CancelCountdownForReconfigure => {}
                        }
                    }
                }
                () = tokio::time::sleep(Duration::from_millis(SERVER_CHECK_INTERVAL_MS)) => {
                    if !session.is_alive() || !is_process_alive(shipping_pid) {
                        break;
                    }
                    let udp: UdpLookupResult = find_udp_listener_pid(current_port, Some(shipping_pid));
                    if udp.available {
                        if udp.pid == Some(shipping_pid) {
                            if !udp_ever_observed {
                                udp_ever_observed = true;
                                {
                                    let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
                                    guard.server_status.udp_confirmed = true;
                                }
                                emit_server_state(&app, &state);
                                emit_server_log(
                                    &app,
                                    &state,
                                    ServerLogLevel::Success,
                                    languages::render_server(&ServerMsg::UdpReadyConfirmed {
                                        pid: shipping_pid,
                                        port: current_port,
                                    }),
                                );
                            }
                            udp_missing_streak = 0;
                        } else if udp_ever_observed {
                            let watchdog_on: bool = state.lock_shared().ui_draft.auto_restart_watchdog;
                            if watchdog_on {
                                udp_missing_streak = udp_missing_streak.saturating_add(1);
                                if udp_missing_streak >= UDP_MISSING_STREAK_LIMIT {
                                    emit_server_log(
                                        &app,
                                        &state,
                                        ServerLogLevel::Warn,
                                        languages::render_server(&ServerMsg::UdpStoppedRestarting {
                                            port: current_port,
                                        }),
                                    );
                                    let _ = terminate_process(shipping_pid);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        if !cancel.is_cancelled() && is_process_alive(shipping_pid) {
            let _ = sleep_abortable(Duration::from_millis(DETACH_EXIT_GRACE_MS), &cancel).await;
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

        if immediate_restart {
            continue;
        }

        let watchdog_on: bool = state.lock_shared().ui_draft.auto_restart_watchdog;
        if !watchdog_on {
            emit_server_log(
                &app,
                &state,
                ServerLogLevel::Info,
                languages::gui_text(GuiMsgCode::WatchdogOffStopped).to_string(),
            );
            break;
        }

        let exit_notice: String = if active_plan.switch_port_on_restart {
            format!("サーバー終了を検知しました (ポート切替: {prev_port} -> {current_port})")
        } else {
            format!("サーバー終了を検知しました (固定ポート維持: {current_port})")
        };
        emit_server_log(&app, &state, ServerLogLevel::Warn, exit_notice);

        let wait_secs: u64 = config.restart_wait_seconds.max(1);
        let mut abort_after_countdown: bool = false;

        for remaining in (1..=wait_secs).rev() {
            {
                let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
                guard.server_status.phase = ServerPhase::Countdown;
                guard.server_status.pid = None;
                guard.server_status.udp_confirmed = false;
                guard.server_status.next_port = Some(current_port);
                guard.server_status.countdown_seconds = Some(remaining);
            }
            emit_server_state(&app, &state);

            let mut break_countdown_now: bool = false;
            let tick_deadline: tokio::time::Instant =
                tokio::time::Instant::now() + Duration::from_secs(1);
            loop {
                tokio::select! {
                    () = cancel.cancelled() => {
                        abort_after_countdown = true;
                        break_countdown_now = true;
                        break;
                    }
                    maybe_cmd = cmd_rx.recv() => {
                        if let Some(cmd) = maybe_cmd {
                            match cmd {
                                ServerControlCommand::RestartNow => {
                                    emit_server_log(
                                        &app,
                                        &state,
                                        ServerLogLevel::Info,
                                        languages::gui_text(GuiMsgCode::ImmediateRestartRequested).to_string(),
                                    );
                                    break_countdown_now = true;
                                    break;
                                }
                                ServerControlCommand::CancelCountdownForReconfigure => {
                                    emit_server_log(
                                        &app,
                                        &state,
                                        ServerLogLevel::Info,
                                        languages::gui_text(GuiMsgCode::CountdownCancelledForSetup).to_string(),
                                    );
                                    abort_after_countdown = true;
                                    break_countdown_now = true;
                                    break;
                                }
                                ServerControlCommand::UpdateNextPlan(new_plan) => {
                                    if let Ok(next_patches) = prepare_injection_plan(
                                        &new_plan.selected_content_option_numbers,
                                        &new_plan.selected_option_numbers,
                                        new_plan.console_patch_port,
                                        None,
                                    ) {
                                        if let Ok(next_dlls) = prepare_server_dll_plan(
                                            &new_plan.selected_content_option_numbers,
                                            None,
                                        ) {
                                            active_plan = new_plan;
                                            patches = next_patches;
                                            dlls = next_dlls;
                                            emit_server_log(
                                                &app,
                                                &state,
                                                ServerLogLevel::Info,
                                                languages::gui_text(GuiMsgCode::NextRoundPlanUpdated).to_string(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    () = tokio::time::sleep_until(tick_deadline) => {
                        break;
                    }
                }
            }

            if break_countdown_now {
                break;
            }
        }

        if abort_after_countdown || cancel.is_cancelled() {
            break;
        }
    }

    {
        let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
        let last_log_file: Option<String> = guard.server_status.current_patch_log_file.clone();
        let mut stopped: ServerMonitorStateDto = idle_server_monitor_state(&guard.config);
        stopped.current_patch_log_file = last_log_file;
        guard.server_status = stopped;
    }
    emit_server_state(&app, &state);
    emit_server_log(
        &app,
        &state,
        ServerLogLevel::Info,
        languages::gui_text(GuiMsgCode::ServerStoppedCleanly).to_string(),
    );
}
