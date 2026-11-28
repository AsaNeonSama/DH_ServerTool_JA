use std::path::{Path, PathBuf};
use std::sync::{Arc, MutexGuard};

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::{mpsc, oneshot, MutexGuard as AsyncMutexGuard};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::config::{save_config as persist_config_file, validate_app_config, AppConfig};
use crate::consts::gui::{EMBEDDED_LICENSE_TEXT, EVENT_CONSOLE_LOG, EVENT_SERVER_STATE};
use crate::gui::console::dispatch_admin_console_action;
use crate::gui::state::{
    build_validated_match_config, check_server_exe_status, normalize_picked_exe_path,
};
use crate::gui::supervisor::run_gui_server_supervisor;
use crate::languages;
use crate::mods::{prepare_injection_plan, ResolvedPatchScript};
use crate::structs::console::{ConsoleBridge, ConsoleLogRecord, ConsoleLogSink};
use crate::structs::gui::{
    ActiveSupervisorHandle, AdminConsoleActionRequestDto, AdminConsoleActionResultDto,
    ConfigSaveResultDto, ConsoleLogEntryDto, GuiAppState, GuiSharedData, GuiSnapshot,
    LaunchServerRequestDto, MatchDraftDto, PickedFileDto, ServerControlCommand,
    ServerMonitorStateDto, ServerPhase, UiSessionDraftDto,
};
use crate::structs::languages::{GuiMsg, GuiMsgCode};
use crate::structs::server::{LaunchPlan, MatchConfig};

fn build_gui_console_log_sink(app: AppHandle, state: GuiAppState) -> ConsoleLogSink {
    Arc::new(move |record: ConsoleLogRecord| {
        let entry: ConsoleLogEntryDto = state.push_console_log(record);
        let _: Result<(), tauri::Error> = app.emit(EVENT_CONSOLE_LOG, entry);
    })
}

fn build_launch_plan_from_state(
    state: &GuiAppState,
    request: &LaunchServerRequestDto,
) -> Result<(AppConfig, LaunchPlan), String> {
    let match_cfg: MatchConfig = build_validated_match_config(request)?;
    let config: AppConfig = state.lock_shared().config.clone();
    validate_app_config(&config)?;

    let server_ports: [u16; 2] = if config.switch_port_on_restart {
        config.server_ports
    } else {
        [config.server_ports[0], config.server_ports[0]]
    };

    let plan: LaunchPlan = LaunchPlan::from_parts(
        config.console_patch_port,
        server_ports,
        config.switch_port_on_restart,
        match_cfg,
    );
    Ok((config, plan))
}

#[tauri::command]
pub async fn get_initial_state(state: State<'_, GuiAppState>) -> Result<GuiSnapshot, String> {
    Ok(state.build_snapshot())
}

#[tauri::command]
pub async fn sync_ui_draft(
    draft: UiSessionDraftDto,
    state: State<'_, GuiAppState>,
) -> Result<(), String> {
    let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
    guard.ui_draft = draft;
    Ok(())
}

#[tauri::command]
pub async fn save_config(
    config: AppConfig,
    auto_restart_watchdog: bool,
    state: State<'_, GuiAppState>,
) -> Result<ConfigSaveResultDto, String> {
    let saved: AppConfig = persist_config_file(&config)?;
    let (server_exe_exists, legacy_frida_dll_detected): (bool, bool) =
        check_server_exe_status(&saved.shipping_executable);

    {
        let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
        guard.config = saved.clone();
        guard.ui_draft.config_draft = saved.clone();
        guard.ui_draft.auto_restart_watchdog = auto_restart_watchdog;
        guard.ui_draft.console_draft.auto_broadcast = saved.auto_broadcast_default;
        guard.server_status.switch_port_on_restart = saved.switch_port_on_restart;
        guard.server_status.server_ports = saved.server_ports;
    }

    Ok(ConfigSaveResultDto {
        config: saved,
        auto_restart_watchdog,
        server_exe_exists,
        legacy_frida_dll_detected,
    })
}

#[tauri::command]
pub async fn reset_default_config() -> Result<AppConfig, String> {
    Ok(AppConfig::default())
}

#[tauri::command]
pub async fn pick_server_executable(app: AppHandle) -> Result<Option<PickedFileDto>, String> {
    let (tx, rx): (
        oneshot::Sender<Option<FilePath>>,
        oneshot::Receiver<Option<FilePath>>,
    ) = oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Windows Executable", &["exe"])
        .pick_file(move |picked: Option<FilePath>| {
            let _: Result<(), Option<FilePath>> = tx.send(picked);
        });

    let Ok(Some(file_path)): Result<Option<FilePath>, oneshot::error::RecvError> = rx.await else {
        return Ok(None);
    };
    let path_buf: PathBuf = match file_path {
        FilePath::Path(pb) => pb,
        FilePath::Url(url) => url
            .to_file_path()
            .map_err(|()| "選択されたURLをローカルパスに変換できませんでした。".to_string())?,
    };
    let cwd: Option<PathBuf> = std::env::current_dir().ok();
    let normalized: String = normalize_picked_exe_path(&path_buf, cwd.as_deref());
    let process_name: String = path_buf
        .file_name()
        .map_or_default(|s: &std::ffi::OsStr| s.to_string_lossy().to_string());
    let exists: bool = path_buf.is_file();

    Ok(Some(PickedFileDto {
        path: normalized,
        process_name,
        exists,
    }))
}

#[tauri::command]
pub async fn execute_admin_console_action(
    request: AdminConsoleActionRequestDto,
    app: AppHandle,
    state: State<'_, GuiAppState>,
) -> Result<AdminConsoleActionResultDto, String> {
    let console_patch_port: u16 = state.lock_shared().config.console_patch_port;
    let bridge: ConsoleBridge = ConsoleBridge::new(console_patch_port);
    let sink: ConsoleLogSink = build_gui_console_log_sink(app, state.inner().clone());
    Ok(dispatch_admin_console_action(&bridge, &sink, &request).await)
}

#[tauri::command]
pub async fn open_logs_folder(app: AppHandle) -> Result<(), String> {
    let cwd: PathBuf = std::env::current_dir()
        .map_err(|_| languages::gui_text(GuiMsgCode::LogDirResolveFailed).to_string())?;
    let cwd_str: String = cwd.to_string_lossy().to_string();
    app.opener()
        .open_path(cwd_str, None::<&str>)
        .map_err(|err: tauri_plugin_opener::Error| {
            let err_s: String = err.to_string();
            languages::render_gui(&GuiMsg::OpenFolderFailed { error_text: &err_s })
        })
}

#[tauri::command]
pub async fn clear_console_logs(state: State<'_, GuiAppState>) -> Result<(), String> {
    state.lock_shared().console_logs.clear();
    Ok(())
}

#[tauri::command]
pub async fn clear_server_logs(state: State<'_, GuiAppState>) -> Result<(), String> {
    state.lock_shared().server_logs.clear();
    Ok(())
}

#[tauri::command]
pub async fn launch_server(
    request: LaunchServerRequestDto,
    app: AppHandle,
    state: State<'_, GuiAppState>,
) -> Result<ServerMonitorStateDto, String> {
    let mut sup_slot: AsyncMutexGuard<'_, Option<ActiveSupervisorHandle>> =
        state.supervisor_handle.lock().await;
    if let Some(existing) = sup_slot.as_ref() {
        if !existing.task.is_finished() {
            return Err(languages::gui_text(GuiMsgCode::ServerAlreadyRunning).to_string());
        }
    }
    let _: Option<ActiveSupervisorHandle> = sup_slot.take();

    let (config, plan): (AppConfig, LaunchPlan) = build_launch_plan_from_state(&state, &request)?;
    if !Path::new(&config.shipping_executable).is_file() {
        return Err(languages::render_gui(&GuiMsg::ServerExeNotFound {
            path: &config.shipping_executable,
        }));
    }

    let patches: Vec<ResolvedPatchScript> = prepare_injection_plan(
        &plan.selected_content_option_numbers,
        &plan.selected_option_numbers,
        plan.console_patch_port,
        None,
    )?;

    {
        let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
        guard.ui_draft.auto_restart_watchdog = request.auto_restart_watchdog;
        guard.ui_draft.match_draft = MatchDraftDto {
            map_id: request.map_id,
            settings: request.settings,
            selected_content_numbers: plan.selected_content_option_numbers.clone(),
            selected_option_numbers: plan.selected_option_numbers.clone(),
        };
        guard.server_status.phase = ServerPhase::Launching;
        guard.server_status.active_port = Some(plan.server_ports[0]);
        guard.server_status.server_ports = plan.server_ports;
        guard.server_status.switch_port_on_restart = plan.switch_port_on_restart;
        guard.server_status.map_id = Some(plan.map_id.to_string());
        guard.server_status.map_label =
            Some(languages::server_text(plan.map_label_code).to_string());
    }

    let cancel: CancellationToken = CancellationToken::new();
    let (cmd_tx, cmd_rx): (
        mpsc::UnboundedSender<ServerControlCommand>,
        mpsc::UnboundedReceiver<ServerControlCommand>,
    ) = mpsc::unbounded_channel();

    let app_clone: AppHandle = app.clone();
    let state_clone: GuiAppState = state.inner().clone();
    let cancel_clone: CancellationToken = cancel.clone();
    let task: JoinHandle<()> = tokio::spawn(async move {
        run_gui_server_supervisor(
            app_clone,
            state_clone,
            plan,
            patches,
            config,
            cancel_clone,
            cmd_rx,
        )
        .await;
    });

    *sup_slot = Some(ActiveSupervisorHandle {
        cancel,
        cmd_tx,
        task,
    });
    drop(sup_slot);

    let snapshot: ServerMonitorStateDto = state.lock_shared().server_status.clone();
    let _: Result<(), tauri::Error> = app.emit(EVENT_SERVER_STATE, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn update_next_round_plan(
    request: LaunchServerRequestDto,
    state: State<'_, GuiAppState>,
) -> Result<String, String> {
    let (_, plan): (AppConfig, LaunchPlan) = build_launch_plan_from_state(&state, &request)?;
    {
        let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
        guard.ui_draft.auto_restart_watchdog = request.auto_restart_watchdog;
        guard.ui_draft.match_draft = MatchDraftDto {
            map_id: request.map_id,
            settings: request.settings,
            selected_content_numbers: plan.selected_content_option_numbers.clone(),
            selected_option_numbers: plan.selected_option_numbers.clone(),
        };
    }

    let sup_slot: AsyncMutexGuard<'_, Option<ActiveSupervisorHandle>> =
        state.supervisor_handle.lock().await;
    let Some(active): Option<&ActiveSupervisorHandle> = sup_slot.as_ref() else {
        return Err(languages::gui_text(GuiMsgCode::ServerNotRunning).to_string());
    };
    active
        .cmd_tx
        .send(ServerControlCommand::UpdateNextPlan(plan))
        .map_err(|_| languages::gui_text(GuiMsgCode::ServerNotRunning).to_string())?;
    Ok(languages::gui_text(GuiMsgCode::NextRoundPlanUpdated).to_string())
}

#[tauri::command]
pub async fn restart_server_now(state: State<'_, GuiAppState>) -> Result<(), String> {
    let sup_slot: AsyncMutexGuard<'_, Option<ActiveSupervisorHandle>> =
        state.supervisor_handle.lock().await;
    let Some(active): Option<&ActiveSupervisorHandle> = sup_slot.as_ref() else {
        return Err(languages::gui_text(GuiMsgCode::ServerNotRunning).to_string());
    };
    active
        .cmd_tx
        .send(ServerControlCommand::RestartNow)
        .map_err(|_| languages::gui_text(GuiMsgCode::ServerNotRunning).to_string())
}

#[tauri::command]
pub async fn cancel_countdown_for_reconfigure(state: State<'_, GuiAppState>) -> Result<(), String> {
    let sup_slot: AsyncMutexGuard<'_, Option<ActiveSupervisorHandle>> =
        state.supervisor_handle.lock().await;
    let Some(active): Option<&ActiveSupervisorHandle> = sup_slot.as_ref() else {
        return Err(languages::gui_text(GuiMsgCode::ServerNotRunning).to_string());
    };
    active
        .cmd_tx
        .send(ServerControlCommand::CancelCountdownForReconfigure)
        .map_err(|_| languages::gui_text(GuiMsgCode::ServerNotRunning).to_string())
}

#[tauri::command]
pub async fn stop_server(app: AppHandle, state: State<'_, GuiAppState>) -> Result<(), String> {
    let existing: Option<ActiveSupervisorHandle> = {
        let mut sup_slot: AsyncMutexGuard<'_, Option<ActiveSupervisorHandle>> =
            state.supervisor_handle.lock().await;
        sup_slot.take()
    };
    let Some(handle): Option<ActiveSupervisorHandle> = existing else {
        return Ok(());
    };

    {
        let mut guard: MutexGuard<'_, GuiSharedData> = state.lock_shared();
        guard.server_status.phase = ServerPhase::Stopping;
    }
    let snapshot: ServerMonitorStateDto = state.lock_shared().server_status.clone();
    let _: Result<(), tauri::Error> = app.emit(EVENT_SERVER_STATE, &snapshot);

    handle.cancel.cancel();
    let _: Result<Result<(), tokio::task::JoinError>, tokio::time::error::Elapsed> =
        tokio::time::timeout(std::time::Duration::from_secs(5), handle.task).await;
    Ok(())
}

#[tauri::command]
pub async fn get_license_text() -> Result<String, String> {
    Ok(EMBEDDED_LICENSE_TEXT.to_string())
}

#[tauri::command]
pub async fn window_minimize(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let _: tauri::Result<()> = window.minimize();
    }
    Ok(())
}

#[tauri::command]
pub async fn window_close(app: AppHandle, state: State<'_, GuiAppState>) -> Result<(), String> {
    shutdown_all_services(state.inner()).await;
    if let Some(window) = app.get_webview_window("main") {
        let _: tauri::Result<()> = window.close();
    }
    Ok(())
}

/// Gracefully shuts down any active game server supervisor on app exit.
pub async fn shutdown_all_services(state: &GuiAppState) {
    let existing_sup: Option<ActiveSupervisorHandle> = {
        let mut sup_slot: AsyncMutexGuard<'_, Option<ActiveSupervisorHandle>> =
            state.supervisor_handle.lock().await;
        sup_slot.take()
    };
    if let Some(handle) = existing_sup {
        handle.cancel.cancel();
        let _: Result<Result<(), tokio::task::JoinError>, tokio::time::error::Elapsed> =
            tokio::time::timeout(std::time::Duration::from_secs(4), handle.task).await;
    }
}
