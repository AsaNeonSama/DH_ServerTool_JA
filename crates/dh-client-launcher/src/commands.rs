use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tokio::sync::oneshot;

use crate::builtin_dlls::builtin_dll_catalog;
use crate::config::{evaluate_file_statuses, normalize_display_path, save_config_in};
use crate::consts::app::{APP_LICENSE_TEXT, APP_LICENSE_URL, APP_VERSION};
use crate::consts::config::EXE_FILE_EXTENSION;
use crate::consts::events::{CATEGORY_SYS, LEVEL_OK};
use crate::consts::messages::{DIALOG_FILTER_EXE, DIALOG_TITLE_EXE};
use crate::discovery::auto_detect_client_exe;
use crate::launcher::{
    clear_buffered_logs, launch_client_process, make_log_event, record_and_emit_log,
    snapshot_buffered_logs, sync_running_game_state, terminate_client_process,
};
use crate::structs::builtin_dlls::BuiltinDllMeta;
use crate::structs::config::{FileStatuses, LauncherConfig};
use crate::structs::discovery::AutoDetectResponse;
use crate::structs::launcher::{LauncherLogEvent, ProcessSnapshot};
use crate::structs::state::{
    AppState, LaunchOutcome, LauncherStateResponse, LogBuffer, TrackedProcess,
};

fn snapshot_state(
    app: Option<&AppHandle>,
    state: &AppState,
) -> Result<LauncherStateResponse, String> {
    let config: LauncherConfig = state
        .config
        .lock()
        .map_err(|_: std::sync::PoisonError<_>| "設定ロックエラー".to_string())?
        .clone();
    let statuses: FileStatuses = evaluate_file_statuses(&state.base_dir, &config);
    let process: ProcessSnapshot =
        sync_running_game_state(app, &state.tracked, &state.logs, &config.exe_path)?;
    let logs: Vec<LauncherLogEvent> = snapshot_buffered_logs(&state.logs);
    let builtin_meta: Vec<BuiltinDllMeta> = builtin_dll_catalog();

    Ok(LauncherStateResponse {
        config,
        statuses,
        process,
        logs,
        builtin_meta,
        license_url: APP_LICENSE_URL.to_string(),
        version: APP_VERSION.to_string(),
    })
}

#[tauri::command]
pub async fn get_initial_state(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LauncherStateResponse, String> {
    snapshot_state(Some(&app), state.inner())
}

#[tauri::command]
pub async fn sync_config_draft(
    state: State<'_, AppState>,
    config: LauncherConfig,
) -> Result<FileStatuses, String> {
    let inner: &AppState = state.inner();
    let statuses: FileStatuses = evaluate_file_statuses(&inner.base_dir, &config);
    {
        let mut guard: std::sync::MutexGuard<'_, LauncherConfig> = inner
            .config
            .lock()
            .map_err(|_: std::sync::PoisonError<_>| "設定ロックエラー".to_string())?;
        *guard = config;
    }
    Ok(statuses)
}

#[tauri::command]
pub async fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: LauncherConfig,
) -> Result<LauncherStateResponse, String> {
    let inner: &AppState = state.inner();
    save_config_in(&inner.base_dir, &config)?;
    {
        let mut guard: std::sync::MutexGuard<'_, LauncherConfig> = inner
            .config
            .lock()
            .map_err(|_: std::sync::PoisonError<_>| "設定ロックエラー".to_string())?;
        *guard = config;
    }
    snapshot_state(Some(&app), inner)
}

#[tauri::command]
pub async fn clear_logs(state: State<'_, AppState>) -> Result<(), String> {
    clear_buffered_logs(&state.inner().logs);
    Ok(())
}

#[tauri::command]
pub async fn refresh_file_statuses(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LauncherStateResponse, String> {
    snapshot_state(Some(&app), state.inner())
}

#[tauri::command]
pub async fn auto_detect_exe(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AutoDetectResponse, String> {
    let inner: &AppState = state.inner();
    let res: AutoDetectResponse = auto_detect_client_exe(&inner.base_dir, true);
    if let Some(ref found_path) = res.found {
        record_and_emit_log(
            &app,
            &inner.logs,
            make_log_event(
                LEVEL_OK,
                CATEGORY_SYS,
                "auto-detect",
                format!("実行ファイルを検出: {found_path}"),
            ),
        );
    }
    Ok(res)
}

#[tauri::command]
pub async fn pick_exe_file(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let base_dir: PathBuf = state.inner().base_dir.clone();
    let (tx, rx): (
        oneshot::Sender<Option<FilePath>>,
        oneshot::Receiver<Option<FilePath>>,
    ) = oneshot::channel();

    app.dialog()
        .file()
        .set_title(DIALOG_TITLE_EXE)
        .add_filter(DIALOG_FILTER_EXE, &[EXE_FILE_EXTENSION])
        .pick_file(move |picked: Option<FilePath>| {
            let _ = tx.send(picked);
        });

    let picked: Option<FilePath> = rx.await.map_err(|_: oneshot::error::RecvError| {
        "ファイル選択ダイアログが中断されました".to_string()
    })?;

    let Some(file_path) = picked else {
        return Ok(None);
    };
    let path_buf: PathBuf = match file_path.into_path() {
        Ok(path) => path,
        Err(err) => return Err(format!("選択パスの解決に失敗しました: {err}")),
    };
    Ok(Some(normalize_display_path(&base_dir, &path_buf)))
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProcessSnapshot, String> {
    let inner: &AppState = state.inner();
    let base_dir: PathBuf = inner.base_dir.clone();
    let config: LauncherConfig = inner
        .config
        .lock()
        .map_err(|_: std::sync::PoisonError<_>| "設定ロックエラー".to_string())?
        .clone();
    let tracked: Arc<std::sync::Mutex<TrackedProcess>> = Arc::clone(&inner.tracked);
    let logs: LogBuffer = Arc::clone(&inner.logs);

    // Run injection on an independent Tokio task so reloading the WebView mid-injection
    // never cancels or interrupts the launch/injection lifecycle.
    let (tx, rx): (
        oneshot::Sender<LaunchOutcome>,
        oneshot::Receiver<LaunchOutcome>,
    ) = oneshot::channel();
    tokio::spawn(async move {
        let outcome: LaunchOutcome =
            launch_client_process(app, base_dir, config, tracked, logs).await;
        let _ = tx.send(outcome);
    });

    rx.await
        .map_err(|_: oneshot::error::RecvError| "起動タスクが中断されました".to_string())?
}

#[tauri::command]
pub async fn terminate_game(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProcessSnapshot, String> {
    let inner: &AppState = state.inner();
    let tracked: Arc<std::sync::Mutex<TrackedProcess>> = Arc::clone(&inner.tracked);
    let logs: LogBuffer = Arc::clone(&inner.logs);
    terminate_client_process(app, tracked, logs).await
}

#[tauri::command]
pub async fn get_process_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProcessSnapshot, String> {
    let inner: &AppState = state.inner();
    let exe_path: String = inner
        .config
        .lock()
        .map_err(|_: std::sync::PoisonError<_>| "設定ロックエラー".to_string())?
        .exe_path
        .clone();
    sync_running_game_state(Some(&app), &inner.tracked, &inner.logs, &exe_path)
}

#[tauri::command]
pub async fn get_license_text() -> Result<String, String> {
    Ok(APP_LICENSE_TEXT.to_string())
}

#[tauri::command]
pub async fn window_start_dragging(window: WebviewWindow) -> Result<(), String> {
    window
        .start_dragging()
        .map_err(|err: tauri::Error| format!("ウィンドウのドラッグ開始に失敗しました: {err}"))
}

#[tauri::command]
pub async fn window_minimize(window: WebviewWindow) -> Result<(), String> {
    window
        .minimize()
        .map_err(|err: tauri::Error| format!("ウィンドウの最小化に失敗しました: {err}"))
}

#[tauri::command]
pub async fn window_close(window: WebviewWindow) -> Result<(), String> {
    window
        .close()
        .map_err(|err: tauri::Error| format!("ウィンドウの終了に失敗しました: {err}"))
}
