pub mod builtin_dlls;
pub mod commands;
pub mod config;
pub mod consts;
pub mod discovery;
pub mod instance_guard;
pub mod launcher;
pub mod structs;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use dh_gadget_injector::sweep_default_stale_run_dirs;
use tauri::Manager;

use crate::config::{load_or_create_config_in, resolve_base_dir};
use crate::consts::app::{CLIENT_APP_INSTANCE_MUTEX_NAME, MAIN_WINDOW_LABEL, MAIN_WINDOW_TITLE};
use crate::consts::events::{CATEGORY_SYS, LEVEL_INFO};
use crate::instance_guard::{focus_existing_window, try_acquire_app_instance};
use crate::launcher::{make_log_event, push_buffered_log, sync_running_game_state};
use crate::structs::config::LauncherConfig;
use crate::structs::state::{AppInstanceGuard, AppState, LogBuffer, TrackedProcess};

#[cfg(windows)]
fn lock_fixed_window_style(window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_MAXIMIZEBOX, WS_THICKFRAME,
    };

    let _ = window.set_resizable(false);
    let _ = window.set_maximizable(false);

    if let Ok(raw_hwnd) = window.hwnd() {
        let hwnd: HWND = HWND(raw_hwnd.0);
        if !hwnd.is_invalid() {
            // SAFETY: `hwnd` is the valid top-level Win32 window handle owned by `window`;
            // clearing `WS_THICKFRAME | WS_MAXIMIZEBOX` and committing with `SWP_FRAMECHANGED`
            // strictly enforces non-resizable, non-maximizable behavior at the OS window level.
            unsafe {
                let current_style: isize = GetWindowLongPtrW(hwnd, GWL_STYLE);
                let forbidden_mask: isize =
                    isize::try_from((WS_THICKFRAME | WS_MAXIMIZEBOX).0).unwrap_or(0);
                let locked_style: isize = current_style & !forbidden_mask;
                if locked_style != current_style {
                    let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, locked_style);
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                    );
                }
            }
        }
    }
}

#[cfg(windows)]
fn configure_main_webview(app: &tauri::App) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Settings, ICoreWebView2Settings3,
    };
    use windows_core::Interface;

    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    lock_fixed_window_style(&window);
    let _ = window.with_webview(|platform_webview: tauri::webview::PlatformWebview| {
        let controller: ICoreWebView2Controller = platform_webview.controller();
        // SAFETY: `controller` is a valid live `ICoreWebView2Controller` provided by Tauri on the
        // `WebView2` UI thread; querying `CoreWebView2`/`Settings` and disabling default context
        // menus, DevTools, and browser accelerator keys are standard COM calls on that thread.
        unsafe {
            if let Ok(core_webview) = controller.CoreWebView2() {
                let webview: ICoreWebView2 = core_webview;
                if let Ok(settings) = webview.Settings() {
                    let base_settings: ICoreWebView2Settings = settings;
                    let _ = base_settings.SetAreDefaultContextMenusEnabled(false);
                    let _ = base_settings.SetAreDevToolsEnabled(false);
                    if let Ok(settings3) = base_settings.cast::<ICoreWebView2Settings3>() {
                        let _ = settings3.SetAreBrowserAcceleratorKeysEnabled(false);
                    }
                }
            }
        }
    });
}

#[cfg(not(windows))]
fn configure_main_webview(_app: &tauri::App) {}

/// Launches the Tauri v2 desktop client launcher window, enforcing a single launcher instance.
pub fn run() -> Result<(), String> {
    let Some(_app_instance_guard): Option<AppInstanceGuard> =
        try_acquire_app_instance(CLIENT_APP_INSTANCE_MUTEX_NAME)?
    else {
        let _focused: bool = focus_existing_window(MAIN_WINDOW_TITLE);
        return Ok(());
    };

    sweep_default_stale_run_dirs();

    let base_dir: PathBuf = resolve_base_dir();
    let initial_config: LauncherConfig =
        load_or_create_config_in(&base_dir).unwrap_or_else(|_: String| LauncherConfig::default());

    let logs: LogBuffer = Arc::new(Mutex::new(VecDeque::new()));
    push_buffered_log(
        &logs,
        make_log_event(
            LEVEL_INFO,
            CATEGORY_SYS,
            "launcher",
            "DH_ClientLauncher_JA の初期化が完了しました。",
        ),
    );

    let initial_exe_path: String = initial_config.exe_path.clone();
    let app_state: AppState = AppState {
        base_dir,
        config: Mutex::new(initial_config),
        tracked: Arc::new(Mutex::new(TrackedProcess::default())),
        logs,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(move |app: &mut tauri::App| {
            configure_main_webview(app);
            let handle: tauri::AppHandle = app.handle().clone();
            let state: tauri::State<'_, AppState> = app.state::<AppState>();
            let _ = sync_running_game_state(
                Some(&handle),
                &state.tracked,
                &state.logs,
                &initial_exe_path,
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_initial_state,
            commands::sync_config_draft,
            commands::save_config,
            commands::clear_logs,
            commands::refresh_file_statuses,
            commands::auto_detect_exe,
            commands::pick_exe_file,
            commands::launch_game,
            commands::terminate_game,
            commands::get_process_status,
            commands::get_license_text,
            commands::window_start_dragging,
            commands::window_minimize,
            commands::window_close,
        ])
        .run(tauri::generate_context!())
        .map_err(|err: tauri::Error| format!("DH_ClientLauncher_JA の起動に失敗しました: {err}"))
}
