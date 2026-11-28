use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dh_gadget_injector::{
    spawn_and_inject_with_cancel, ActiveInjectionSession, DllEventCallback, DllInjectionEvent,
    DllLoadResult, EventCallback, InjectLaunchRequest, InjectorEvent, LogLevel, OepStubInjector,
    PatchMod, MAX_INJECT_DLLS,
};
use dh_win32::{get_local_time, terminate_process, LocalTimeParts, ProcessInfo};
use tauri::{AppHandle, Emitter};
use tokio::time::Interval;

use crate::builtin_dlls::{extract_enabled_builtin_dlls, selected_builtin_ids};
use crate::config::{describe_launch_mode, resolve_entry_path};
use crate::consts::app::{
    CLIENT_INIT_TIMEOUT_SECS, CLIENT_LAUNCH_MUTEX_NAME, CLIENT_PATCH_LOG_RELATIVE_PATH,
    DREAD_HUNGER_STEAM_APP_ID, ENV_STEAM_APP_ID, ENV_STEAM_GAME_ID, LAUNCH_MUTEX_WAIT_MS,
    MAX_BUFFERED_LOGS, PROCESS_POLL_INTERVAL_MS,
};
use crate::consts::events::{
    CATEGORY_DLL, CATEGORY_JS, CATEGORY_PROC, CATEGORY_SYS, EVENT_LAUNCHER_LOG,
    EVENT_LAUNCHER_STATUS, LEVEL_ERR, LEVEL_INFO, LEVEL_OK, LEVEL_WARN,
};
use crate::consts::messages::{
    ERR_EXE_PATH_EMPTY, ERR_LAUNCH_IN_PROGRESS, ERR_NO_RUNNING_PROCESS, ERR_TERMINATE_FAILED,
    MODE_EXTERNAL_RUNNING, MODE_PLAIN,
};
use crate::instance_guard::{detect_running_game_process, is_pid_running, try_acquire_named_mutex};
use crate::structs::builtin_dlls::{BuiltinDllId, ExtractedBuiltinDll};
use crate::structs::config::LauncherConfig;
use crate::structs::launcher::{
    LauncherLogEvent, PreparedLaunchInputs, ProcessLifecycle, ProcessSnapshot,
};
use crate::structs::state::{LogBuffer, NamedMutexGuard, ReconcileOutcome, TrackedProcess};

impl Default for ProcessSnapshot {
    fn default() -> Self {
        Self {
            status: ProcessLifecycle::Idle,
            pid: None,
            exit_code: None,
            mode_label: MODE_PLAIN.to_string(),
        }
    }
}

/// Returns the child-process environment variable overrides required for `steam_api64.dll`
/// (`SteamAPI_Init`) to authenticate Dread Hunger (`AppID 1418630`) with the running Steam client
/// when launching `DreadHunger-Win64-Shipping.exe` directly without touching any game files.
#[must_use]
pub fn steam_launch_env_overrides() -> Vec<(String, String)> {
    vec![
        (
            ENV_STEAM_APP_ID.to_string(),
            DREAD_HUNGER_STEAM_APP_ID.to_string(),
        ),
        (
            ENV_STEAM_GAME_ID.to_string(),
            DREAD_HUNGER_STEAM_APP_ID.to_string(),
        ),
    ]
}

/// Formats the current local timestamp as `HH:MM:SS.mmm` for log entries.
#[must_use]
pub fn format_log_timestamp() -> String {
    let now: LocalTimeParts = get_local_time();
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        now.hour, now.minute, now.second, now.millisecond
    )
}

/// Constructs a structured [`LauncherLogEvent`] with the current local timestamp.
#[must_use]
pub fn make_log_event(
    level: &str,
    category: &str,
    source: &str,
    message: impl Into<String>,
) -> LauncherLogEvent {
    LauncherLogEvent {
        timestamp: format_log_timestamp(),
        level: level.to_string(),
        category: category.to_string(),
        source: source.to_string(),
        message: message.into(),
    }
}

/// Appends `event` to the bounded backend log ring buffer retained across `WebView` reloads.
pub fn push_buffered_log(logs: &LogBuffer, event: LauncherLogEvent) {
    if let Ok(mut guard) = logs.lock() {
        let queue: &mut VecDeque<LauncherLogEvent> = &mut guard;
        if queue.len() >= MAX_BUFFERED_LOGS {
            let _ = queue.pop_front();
        }
        queue.push_back(event);
    }
}

/// Returns a cloned snapshot of all currently buffered backend log entries.
#[must_use]
pub fn snapshot_buffered_logs(logs: &LogBuffer) -> Vec<LauncherLogEvent> {
    match logs.lock() {
        Ok(guard) => guard.iter().cloned().collect(),
        Err(_) => Vec::new(),
    }
}

/// Clears all buffered backend log entries.
pub fn clear_buffered_logs(logs: &LogBuffer) {
    if let Ok(mut guard) = logs.lock() {
        guard.clear();
    }
}

/// Emits a structured log line to the `WebView`.
pub fn emit_log(app: &AppHandle, event: &LauncherLogEvent) {
    let _ = app.emit(EVENT_LAUNCHER_LOG, event);
}

/// Records `event` into the backend reload-resilient ring buffer and emits it to the `WebView`.
pub fn record_and_emit_log(app: &AppHandle, logs: &LogBuffer, event: LauncherLogEvent) {
    emit_log(app, &event);
    push_buffered_log(logs, event);
}

/// Emits a [`ProcessSnapshot`] update to the `WebView`.
pub fn emit_status(app: &AppHandle, snapshot: &ProcessSnapshot) {
    let _ = app.emit(EVENT_LAUNCHER_STATUS, snapshot);
}

/// Formats the user-facing error returned when launching is blocked because a game instance is
/// already running.
#[must_use]
pub fn format_already_running_error(pid: u32) -> String {
    format!("ゲームはすでに起動中です (PID: {pid}) — 多重起動は許可されていません")
}

/// Attempts to claim the in-process launch slot inside `tracked`.
///
/// Rejects immediately if another launch is already in-flight or if a tracked owned/external game
/// process is still alive (synchronizing state if an external PID was already recorded).
pub fn try_begin_launch_slot(
    tracked: &Arc<Mutex<TrackedProcess>>,
    mode_label: &str,
) -> Result<(u64, Option<ActiveInjectionSession>), String> {
    let mut guard: std::sync::MutexGuard<'_, TrackedProcess> = tracked
        .lock()
        .map_err(|_: std::sync::PoisonError<_>| "内部状態ロックエラー".to_string())?;

    if guard.launching {
        return Err(ERR_LAUNCH_IN_PROGRESS.to_string());
    }

    if let Some(ref active_session) = guard.session {
        if active_session.is_alive() {
            let active_pid: u32 = active_session.pid();
            guard.snapshot.status = ProcessLifecycle::Running;
            guard.snapshot.pid = Some(active_pid);
            guard.snapshot.exit_code = None;
            return Err(format_already_running_error(active_pid));
        }
    }

    if let Some(ext_pid) = guard.external_pid {
        if is_pid_running(ext_pid) {
            guard.snapshot.status = ProcessLifecycle::Running;
            guard.snapshot.pid = Some(ext_pid);
            guard.snapshot.exit_code = None;
            return Err(format_already_running_error(ext_pid));
        }
        guard.external_pid = None;
    }

    guard.launching = true;
    guard.generation = guard.generation.saturating_add(1);
    let next_gen: u64 = guard.generation;
    guard.snapshot = ProcessSnapshot {
        status: ProcessLifecycle::Injecting,
        pid: None,
        exit_code: None,
        mode_label: mode_label.to_string(),
    };
    let prev: Option<ActiveInjectionSession> = guard.session.take();
    Ok((next_gen, prev))
}

/// Pure state-reconciliation helper that updates `tracked` against an observed OS-wide running
/// game process (without touching Tauri runtime handles).
pub fn reconcile_running_game_state(
    tracked: &Arc<Mutex<TrackedProcess>>,
    detected: Option<&ProcessInfo>,
) -> Result<ReconcileOutcome, String> {
    let mut guard: std::sync::MutexGuard<'_, TrackedProcess> = tracked
        .lock()
        .map_err(|_: std::sync::PoisonError<_>| "プロセス状態ロックエラー".to_string())?;

    if guard.launching {
        return Ok(ReconcileOutcome {
            snapshot: guard.snapshot.clone(),
            watch_target: None,
            changed: false,
        });
    }
    if let Some(ref session) = guard.session {
        if session.is_alive() {
            return Ok(ReconcileOutcome {
                snapshot: guard.snapshot.clone(),
                watch_target: None,
                changed: false,
            });
        }
    }

    if let Some(proc_info) = detected {
        let already_tracking: bool = guard.snapshot.status == ProcessLifecycle::Running
            && guard.snapshot.pid == Some(proc_info.pid)
            && guard.external_pid == Some(proc_info.pid);
        if already_tracking {
            Ok(ReconcileOutcome {
                snapshot: guard.snapshot.clone(),
                watch_target: None,
                changed: false,
            })
        } else {
            guard.generation = guard.generation.saturating_add(1);
            let next_gen: u64 = guard.generation;
            guard.external_pid = Some(proc_info.pid);
            guard.snapshot = ProcessSnapshot {
                status: ProcessLifecycle::Running,
                pid: Some(proc_info.pid),
                exit_code: None,
                mode_label: MODE_EXTERNAL_RUNNING.to_string(),
            };
            Ok(ReconcileOutcome {
                snapshot: guard.snapshot.clone(),
                watch_target: Some((next_gen, proc_info.pid)),
                changed: true,
            })
        }
    } else if guard.external_pid.is_some() || guard.snapshot.status == ProcessLifecycle::Running {
        guard.external_pid = None;
        guard.snapshot.status = ProcessLifecycle::Idle;
        guard.snapshot.pid = None;
        guard.snapshot.exit_code = None;
        Ok(ReconcileOutcome {
            snapshot: guard.snapshot.clone(),
            watch_target: None,
            changed: true,
        })
    } else {
        Ok(ReconcileOutcome {
            snapshot: guard.snapshot.clone(),
            watch_target: None,
            changed: false,
        })
    }
}

/// Synchronizes `tracked` with any already-running OS-wide Dread Hunger game process (e.g.
/// detached from a previous launcher run or launched externally via Steam) so startup, reload,
/// and status refresh always reflect true single-instance state.
pub fn sync_running_game_state(
    app: Option<&AppHandle>,
    tracked: &Arc<Mutex<TrackedProcess>>,
    logs: &LogBuffer,
    configured_exe_path: &str,
) -> Result<ProcessSnapshot, String> {
    {
        let guard: std::sync::MutexGuard<'_, TrackedProcess> = tracked
            .lock()
            .map_err(|_: std::sync::PoisonError<_>| "プロセス状態ロックエラー".to_string())?;
        if guard.launching {
            return Ok(guard.snapshot.clone());
        }
        if let Some(ref session) = guard.session {
            if session.is_alive() {
                return Ok(guard.snapshot.clone());
            }
        }
    }

    let detected: Option<ProcessInfo> = detect_running_game_process(configured_exe_path);
    let ReconcileOutcome {
        snapshot,
        watch_target: newly_watched_pid,
        changed: should_emit,
    } = reconcile_running_game_state(tracked, detected.as_ref())?;

    if let Some(handle) = app {
        if should_emit {
            emit_status(handle, &snapshot);
        }
        if let Some((gen_id, ext_pid)) = newly_watched_pid {
            record_and_emit_log(
                handle,
                logs,
                make_log_event(
                    LEVEL_INFO,
                    CATEGORY_PROC,
                    "tracker",
                    format!(
                        "起動中のゲームプロセスを検出しました (PID: {ext_pid}) — 多重起動をロックしました"
                    ),
                ),
            );
            spawn_external_exit_watcher(
                handle.clone(),
                Arc::clone(tracked),
                Arc::clone(logs),
                gen_id,
                ext_pid,
            );
        }
    }

    Ok(snapshot)
}

/// Validates the configured target executable and prepares resolved inputs (including
/// built-in DLL selections, future built-in JS mods slot, and Steam `AppID` env overrides)
/// required for [`InjectLaunchRequest::for_gui_client`].
pub fn prepare_launch_inputs(
    base_dir: &Path,
    config: &LauncherConfig,
) -> Result<PreparedLaunchInputs, String> {
    let exe_trimmed: &str = config.exe_path.trim();
    if exe_trimmed.is_empty() {
        return Err(ERR_EXE_PATH_EMPTY.to_string());
    }
    let exe_path: PathBuf = resolve_entry_path(base_dir, exe_trimmed);
    if !exe_path.is_file() {
        return Err(format!(
            "実行ファイルが見つかりません: {}",
            exe_path.display()
        ));
    }

    let builtin_ids: Vec<BuiltinDllId> = selected_builtin_ids(&config.builtin);
    // Reserved for future curated built-in JS MODs; empty today so zero Frida Gadget overhead.
    let mods: Vec<PatchMod> = Vec::new();

    let max_dlls: usize = if mods.is_empty() {
        MAX_INJECT_DLLS
    } else {
        MAX_INJECT_DLLS.saturating_sub(1)
    };
    if builtin_ids.len() > max_dlls {
        return Err(format!(
            "有効な内蔵 DLL 数 ({}) が上限 ({max_dlls}) を超えています。",
            builtin_ids.len()
        ));
    }

    // Always launch the game client with default empty command-line arguments.
    let args: Vec<String> = Vec::new();
    let env_overrides: Vec<(String, String)> = steam_launch_env_overrides();
    let mode_label: String = describe_launch_mode(builtin_ids.len(), mods.len());
    Ok(PreparedLaunchInputs {
        exe_path,
        args,
        builtin_ids,
        mods,
        env_overrides,
        mode_label,
    })
}

fn file_display_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n: &std::ffi::OsStr| n.to_string_lossy().to_string(),
    )
}

/// Spawns `DreadHunger-Win64-Shipping.exe` with enabled built-in embedded DLLs (EOS Fix, Voice Chat)
/// and Steam `AppID 1418630` child environment overrides, enforcing strict OS-wide single-instance
/// protection via a named Windows mutex + Toolhelp32 process check, streaming live events to the
/// `WebView` without attaching a kill-on-close job.
pub async fn launch_client_process(
    app: AppHandle,
    base_dir: PathBuf,
    config: LauncherConfig,
    tracked: Arc<Mutex<TrackedProcess>>,
    logs: LogBuffer,
) -> Result<ProcessSnapshot, String> {
    let prepared: PreparedLaunchInputs = prepare_launch_inputs(&base_dir, &config)?;
    let PreparedLaunchInputs {
        exe_path,
        args,
        builtin_ids: _,
        mods,
        env_overrides,
        mode_label,
    } = prepared;

    // 1. Acquire in-process launch guard immediately to prevent double-click races.
    let (generation, previous_session): (u64, Option<ActiveInjectionSession>) =
        match try_begin_launch_slot(&tracked, &mode_label) {
            Ok(slot) => slot,
            Err(err_msg) => {
                if let Ok(guard) = tracked.lock() {
                    emit_status(&app, &guard.snapshot);
                }
                record_and_emit_log(
                    &app,
                    &logs,
                    make_log_event(LEVEL_WARN, CATEGORY_PROC, "guard", err_msg.clone()),
                );
                return Err(err_msg);
            }
        };

    // 2. Acquire OS-wide Windows Named Mutex so two separate launcher windows cannot race.
    let _os_launch_guard: NamedMutexGuard =
        match try_acquire_named_mutex(CLIENT_LAUNCH_MUTEX_NAME, LAUNCH_MUTEX_WAIT_MS) {
            Ok(Some(guard)) => guard,
            Ok(None) => {
                reset_failed_launch(
                    &app,
                    &tracked,
                    &logs,
                    generation,
                    mode_label,
                    ERR_LAUNCH_IN_PROGRESS,
                );
                return Err(ERR_LAUNCH_IN_PROGRESS.to_string());
            }
            Err(err_msg) => {
                reset_failed_launch(&app, &tracked, &logs, generation, mode_label, &err_msg);
                return Err(err_msg);
            }
        };

    // 3. Probe OS-wide for any already-running Dread Hunger instance (detached, reopened launcher,
    //    or external Steam launch) while holding the OS-wide launch mutex.
    if let Some(existing_proc) = detect_running_game_process(&config.exe_path) {
        let existing_pid: u32 = existing_proc.pid;
        let warn_msg: String = format_already_running_error(existing_pid);
        let snap: ProcessSnapshot = ProcessSnapshot {
            status: ProcessLifecycle::Running,
            pid: Some(existing_pid),
            exit_code: None,
            mode_label: MODE_EXTERNAL_RUNNING.to_string(),
        };
        let next_gen: u64 = {
            let mut guard: std::sync::MutexGuard<'_, TrackedProcess> = tracked
                .lock()
                .map_err(|_: std::sync::PoisonError<_>| "内部状態ロックエラー".to_string())?;
            guard.launching = false;
            guard.generation = guard.generation.saturating_add(1);
            guard.external_pid = Some(existing_pid);
            guard.snapshot = snap.clone();
            emit_status(&app, &guard.snapshot);
            guard.generation
        };
        if let Some(dead_prev) = previous_session {
            dead_prev.detach().await;
        }
        record_and_emit_log(
            &app,
            &logs,
            make_log_event(LEVEL_WARN, CATEGORY_PROC, "guard", warn_msg.clone()),
        );
        spawn_external_exit_watcher(app, Arc::clone(&tracked), logs, next_gen, existing_pid);
        return Err(warn_msg);
    }

    if let Ok(guard) = tracked.lock() {
        emit_status(&app, &guard.snapshot);
    }

    if let Some(dead_prev) = previous_session {
        dead_prev.detach().await;
    }

    let extracted_builtins: Vec<ExtractedBuiltinDll> =
        match extract_enabled_builtin_dlls(&base_dir, &config.builtin) {
            Ok(list) => list,
            Err(err_msg) => {
                reset_failed_launch(&app, &tracked, &logs, generation, mode_label, &err_msg);
                return Err(err_msg);
            }
        };

    let mut ordered_dll_paths: Vec<PathBuf> = Vec::with_capacity(extracted_builtins.len());
    for item in &extracted_builtins {
        let short_sha: &str = item.sha256.get(..12).unwrap_or(item.sha256);
        let state_label: &str = if item.cache_hit {
            "SHA-256確認済キャッシュ"
        } else {
            "内蔵DLLを展開・SHA-256検証完了"
        };
        record_and_emit_log(
            &app,
            &logs,
            make_log_event(
                LEVEL_INFO,
                CATEGORY_DLL,
                "builtin",
                format!("{} — {state_label} ({short_sha})", item.label),
            ),
        );
        ordered_dll_paths.push(item.path.clone());
    }

    record_and_emit_log(
        &app,
        &logs,
        make_log_event(
            LEVEL_INFO,
            CATEGORY_SYS,
            "steam",
            format!(
                "Steam 連携環境変数を設定 ({ENV_STEAM_APP_ID}={DREAD_HUNGER_STEAM_APP_ID}, {ENV_STEAM_GAME_ID}={DREAD_HUNGER_STEAM_APP_ID})"
            ),
        ),
    );

    record_and_emit_log(
        &app,
        &logs,
        make_log_event(
            LEVEL_INFO,
            CATEGORY_SYS,
            "launch",
            format!("起動開始: {} [{}]", exe_path.display(), mode_label),
        ),
    );

    let dll_count: usize = ordered_dll_paths.len();
    let js_count: usize = mods.len();

    let mut req: InjectLaunchRequest =
        InjectLaunchRequest::for_gui_client(exe_path.clone(), args, ordered_dll_paths, mods);
    req.cwd = exe_path.parent().map(Path::to_path_buf);
    req.env_overrides = env_overrides;
    req.patch_log_path = resolve_entry_path(&base_dir, CLIENT_PATCH_LOG_RELATIVE_PATH);
    req.round = generation;
    req.init_timeout = Duration::from_secs(CLIENT_INIT_TIMEOUT_SECS);

    let app_for_dll: AppHandle = app.clone();
    let logs_for_dll: LogBuffer = Arc::clone(&logs);
    let dll_cb: DllEventCallback = Arc::new(move |event: DllInjectionEvent| match event {
        DllInjectionEvent::Loaded {
            index,
            total,
            path,
            module_handle,
        } => {
            let src: String = file_display_name(&path);
            let msg: String = format!(
                "[{}/{total}] OEP DLL 注入成功 (HMODULE: {module_handle:#x})",
                index.saturating_add(1)
            );
            record_and_emit_log(
                &app_for_dll,
                &logs_for_dll,
                make_log_event(LEVEL_OK, CATEGORY_DLL, &src, msg),
            );
        }
        DllInjectionEvent::Failed { index, total, path } => {
            let src: String = file_display_name(&path);
            let msg: String = format!(
                "[{}/{total}] OEP DLL 注入失敗 (LoadLibraryW が NULL を返しました)",
                index.saturating_add(1)
            );
            record_and_emit_log(
                &app_for_dll,
                &logs_for_dll,
                make_log_event(LEVEL_ERR, CATEGORY_DLL, &src, msg),
            );
        }
    });
    req.on_dll_event = Some(dll_cb);

    let app_for_js: AppHandle = app.clone();
    let logs_for_js: LogBuffer = Arc::clone(&logs);
    let js_cb: EventCallback = Arc::new(move |event: InjectorEvent| match event {
        InjectorEvent::ModLoaded { file_name } => {
            record_and_emit_log(
                &app_for_js,
                &logs_for_js,
                make_log_event(
                    LEVEL_OK,
                    CATEGORY_JS,
                    &file_name,
                    "Frida JS 注入完了 (MOD_OK)",
                ),
            );
        }
        InjectorEvent::Log {
            level,
            file_name,
            message,
        } => {
            let level_str: &str = match level {
                LogLevel::Info => LEVEL_INFO,
                LogLevel::Warn => LEVEL_WARN,
                LogLevel::Error => LEVEL_ERR,
            };
            record_and_emit_log(
                &app_for_js,
                &logs_for_js,
                make_log_event(level_str, CATEGORY_JS, &file_name, message),
            );
        }
        InjectorEvent::InitOk => {
            record_and_emit_log(
                &app_for_js,
                &logs_for_js,
                make_log_event(
                    LEVEL_OK,
                    CATEGORY_JS,
                    "frida-gadget",
                    "全 JS スクリプト初期化完了 (INIT_OK)",
                ),
            );
        }
        InjectorEvent::InitError { file_name, message } => {
            record_and_emit_log(
                &app_for_js,
                &logs_for_js,
                make_log_event(
                    LEVEL_ERR,
                    CATEGORY_JS,
                    &file_name,
                    format!("JS 初期化エラー (INIT_ERR): {message}"),
                ),
            );
        }
        InjectorEvent::ChannelClosed => {
            record_and_emit_log(
                &app_for_js,
                &logs_for_js,
                make_log_event(
                    LEVEL_INFO,
                    CATEGORY_JS,
                    "frida-gadget",
                    "Frida ログチャネルがクローズされました",
                ),
            );
        }
    });
    req.on_event = Some(js_cb);

    let injector: OepStubInjector = OepStubInjector;
    let session: ActiveInjectionSession =
        match spawn_and_inject_with_cancel(&injector, req, std::future::pending()).await {
            Ok(sess) => sess,
            Err(err) => {
                let err_msg: String = format!("{err}");
                reset_failed_launch(&app, &tracked, &logs, generation, mode_label, &err_msg);
                return Err(err_msg);
            }
        };

    let pid: u32 = session.pid();
    let results: &[DllLoadResult] = session.dll_results();
    let loaded_dlls: usize = results
        .iter()
        .filter(|r: &&DllLoadResult| r.is_loaded())
        .count();

    if dll_count > 0 {
        let summary_level: &str = if loaded_dlls == dll_count {
            LEVEL_OK
        } else {
            LEVEL_WARN
        };
        record_and_emit_log(
            &app,
            &logs,
            make_log_event(
                summary_level,
                CATEGORY_DLL,
                "summary",
                format!("内蔵 DLL 注入集計: 成功 {loaded_dlls} / {dll_count} 件"),
            ),
        );
    }

    record_and_emit_log(
        &app,
        &logs,
        make_log_event(
            LEVEL_OK,
            CATEGORY_PROC,
            "process",
            format!("ゲーム起動完了 (PID: {pid} / 内蔵DLL: {dll_count}件 / 内蔵JS: {js_count}件)"),
        ),
    );

    let snapshot: ProcessSnapshot = ProcessSnapshot {
        status: ProcessLifecycle::Running,
        pid: Some(pid),
        exit_code: None,
        mode_label,
    };

    {
        let mut guard: std::sync::MutexGuard<'_, TrackedProcess> = tracked
            .lock()
            .map_err(|_: std::sync::PoisonError<_>| "内部状態ロックエラー".to_string())?;
        guard.launching = false;
        guard.external_pid = None;
        guard.snapshot = snapshot.clone();
        guard.session = Some(session);
        emit_status(&app, &guard.snapshot);
    }

    spawn_exit_watcher(app, Arc::clone(&tracked), logs, generation, pid);
    Ok(snapshot)
}

fn reset_failed_launch(
    app: &AppHandle,
    tracked: &Arc<Mutex<TrackedProcess>>,
    logs: &LogBuffer,
    generation: u64,
    mode_label: String,
    err_msg: &str,
) {
    if let Ok(mut guard) = tracked.lock() {
        if guard.generation == generation {
            guard.launching = false;
            guard.external_pid = None;
            guard.snapshot = ProcessSnapshot {
                status: ProcessLifecycle::Idle,
                pid: None,
                exit_code: None,
                mode_label,
            };
            emit_status(app, &guard.snapshot);
        }
    }
    record_and_emit_log(
        app,
        logs,
        make_log_event(
            LEVEL_ERR,
            CATEGORY_SYS,
            "launch",
            format!("起動エラー: {err_msg}"),
        ),
    );
}

fn spawn_exit_watcher(
    app: AppHandle,
    tracked: Arc<Mutex<TrackedProcess>>,
    logs: LogBuffer,
    generation: u64,
    pid: u32,
) {
    let _task: tauri::async_runtime::JoinHandle<()> = tauri::async_runtime::spawn(async move {
        let mut poll: Interval =
            tokio::time::interval(Duration::from_millis(PROCESS_POLL_INTERVAL_MS));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            poll.tick().await;
            let exit_outcome: Option<Option<u32>> = {
                let Ok(guard): Result<
                    std::sync::MutexGuard<'_, TrackedProcess>,
                    std::sync::PoisonError<_>,
                > = tracked.lock() else {
                    break;
                };
                if guard.generation != generation {
                    break;
                }
                let Some(ref session) = guard.session else {
                    break;
                };

                match session.try_exit_code() {
                    Ok(Some(code)) => Some(Some(code)),
                    Ok(None) | Err(_) => {
                        if session.is_alive() {
                            None
                        } else {
                            Some(None)
                        }
                    }
                }
            };

            if let Some(exit_code) = exit_outcome {
                let finished_session: Option<ActiveInjectionSession> = {
                    let Ok(mut guard): Result<
                        std::sync::MutexGuard<'_, TrackedProcess>,
                        std::sync::PoisonError<_>,
                    > = tracked.lock() else {
                        break;
                    };
                    if guard.generation != generation {
                        break;
                    }
                    guard.snapshot.status = ProcessLifecycle::Exited;
                    guard.snapshot.exit_code = exit_code;
                    emit_status(&app, &guard.snapshot);
                    guard.session.take()
                };

                if let Some(sess) = finished_session {
                    sess.detach().await;
                }

                let code_desc: String =
                    exit_code.map_or_else(|| "不明".to_string(), |code: u32| code.to_string());
                record_and_emit_log(
                    &app,
                    &logs,
                    make_log_event(
                        LEVEL_INFO,
                        CATEGORY_PROC,
                        "process",
                        format!(
                            "ゲームプロセスが終了しました (PID: {pid} / 終了コード: {code_desc})"
                        ),
                    ),
                );
                break;
            }
        }
    });
}

fn spawn_external_exit_watcher(
    app: AppHandle,
    tracked: Arc<Mutex<TrackedProcess>>,
    logs: LogBuffer,
    generation: u64,
    pid: u32,
) {
    let _task: tauri::async_runtime::JoinHandle<()> = tauri::async_runtime::spawn(async move {
        let mut poll: Interval =
            tokio::time::interval(Duration::from_millis(PROCESS_POLL_INTERVAL_MS));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            poll.tick().await;
            let still_tracked: bool = {
                let Ok(guard): Result<
                    std::sync::MutexGuard<'_, TrackedProcess>,
                    std::sync::PoisonError<_>,
                > = tracked.lock() else {
                    break;
                };
                guard.generation == generation && guard.external_pid == Some(pid)
            };
            if !still_tracked {
                break;
            }

            if !is_pid_running(pid) {
                let updated: bool = {
                    let Ok(mut guard): Result<
                        std::sync::MutexGuard<'_, TrackedProcess>,
                        std::sync::PoisonError<_>,
                    > = tracked.lock() else {
                        break;
                    };
                    if guard.generation != generation || guard.external_pid != Some(pid) {
                        break;
                    }
                    guard.external_pid = None;
                    guard.snapshot.status = ProcessLifecycle::Exited;
                    guard.snapshot.exit_code = None;
                    emit_status(&app, &guard.snapshot);
                    true
                };
                if updated {
                    record_and_emit_log(
                        &app,
                        &logs,
                        make_log_event(
                            LEVEL_INFO,
                            CATEGORY_PROC,
                            "process",
                            format!(
                                "既存ゲームプロセスが終了しました (PID: {pid}) — 再起動可能です"
                            ),
                        ),
                    );
                }
                break;
            }
        }
    });
}

/// Terminates the currently tracked game process (owned session or detected external instance)
/// on explicit user request.
pub async fn terminate_client_process(
    app: AppHandle,
    tracked: Arc<Mutex<TrackedProcess>>,
    logs: LogBuffer,
) -> Result<ProcessSnapshot, String> {
    let (pid, session_to_detach, snapshot): (u32, Option<ActiveInjectionSession>, ProcessSnapshot) = {
        let mut guard: std::sync::MutexGuard<'_, TrackedProcess> = tracked
            .lock()
            .map_err(|_: std::sync::PoisonError<_>| "内部状態ロックエラー".to_string())?;
        if let Some(ref session) = guard.session {
            if session.is_alive() {
                let target_pid: u32 = session.pid();
                session
                    .terminate(1)
                    .map_err(|err: dh_gadget_injector::InjectorError| {
                        format!("{ERR_TERMINATE_FAILED} ({err})")
                    })?;
                guard.generation = guard.generation.saturating_add(1);
                guard.external_pid = None;
                guard.snapshot.status = ProcessLifecycle::Exited;
                guard.snapshot.exit_code = Some(1);
                let snap: ProcessSnapshot = guard.snapshot.clone();
                let sess: Option<ActiveInjectionSession> = guard.session.take();
                emit_status(&app, &snap);
                (target_pid, sess, snap)
            } else {
                return Err(ERR_NO_RUNNING_PROCESS.to_string());
            }
        } else if let Some(ext_pid) = guard.external_pid {
            if !is_pid_running(ext_pid) {
                guard.external_pid = None;
                guard.snapshot.status = ProcessLifecycle::Idle;
                guard.snapshot.pid = None;
                guard.snapshot.exit_code = None;
                emit_status(&app, &guard.snapshot);
                return Err(ERR_NO_RUNNING_PROCESS.to_string());
            }
            if !terminate_process(ext_pid) {
                return Err(ERR_TERMINATE_FAILED.to_string());
            }
            guard.generation = guard.generation.saturating_add(1);
            guard.external_pid = None;
            guard.snapshot.status = ProcessLifecycle::Exited;
            guard.snapshot.exit_code = Some(1);
            let snap: ProcessSnapshot = guard.snapshot.clone();
            emit_status(&app, &snap);
            (ext_pid, None, snap)
        } else {
            return Err(ERR_NO_RUNNING_PROCESS.to_string());
        }
    };

    if let Some(sess) = session_to_detach {
        sess.detach().await;
    }

    record_and_emit_log(
        &app,
        &logs,
        make_log_event(
            LEVEL_WARN,
            CATEGORY_PROC,
            "process",
            format!("ゲームプロセスを強制終了しました (PID: {pid})"),
        ),
    );
    Ok(snapshot)
}

/// Counts enabled built-in DLLs in `config`.
#[must_use]
pub fn count_enabled_builtins(config: &LauncherConfig) -> usize {
    selected_builtin_ids(&config.builtin).len()
}
