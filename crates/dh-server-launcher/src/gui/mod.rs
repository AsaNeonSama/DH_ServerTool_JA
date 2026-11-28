pub mod commands;
pub mod console;
pub mod state;
pub mod supervisor;

use tauri::{App, AppHandle, Manager, RunEvent};

use crate::gui::commands::shutdown_all_services;
use crate::structs::gui::GuiAppState;

#[cfg(windows)]
fn lock_fixed_window_style(main_window: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_MAXIMIZEBOX, WS_THICKFRAME,
    };

    let _: tauri::Result<()> = main_window.set_resizable(false);
    let _: tauri::Result<()> = main_window.set_maximizable(false);

    if let Ok(tauri_hwnd) = main_window.hwnd() {
        let hwnd: HWND = HWND(tauri_hwnd.0);
        if let Ok(forbidden_bits) = isize::try_from(WS_THICKFRAME.0 | WS_MAXIMIZEBOX.0) {
            // SAFETY: `hwnd` is the live Win32 handle of the Tauri main window created on this
            // STA thread. Clearing `WS_THICKFRAME` and `WS_MAXIMIZEBOX` via `SetWindowLongPtrW`
            // followed by `SetWindowPos(SWP_FRAMECHANGED)` enforces a fixed non-resizable,
            // non-maximizable window even on frameless DWM windows.
            unsafe {
                let current_style: isize = GetWindowLongPtrW(hwnd, GWL_STYLE);
                let locked_style: isize = current_style & !forbidden_bits;
                if locked_style != current_style {
                    let _: isize = SetWindowLongPtrW(hwnd, GWL_STYLE, locked_style);
                    let _: windows::core::Result<()> = SetWindowPos(
                        hwnd,
                        HWND::default(),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                    );
                }
            }
        }
    }
}

#[cfg(windows)]
fn harden_windows_webview(app: &App) {
    use tauri::webview::PlatformWebview;
    use tauri::WebviewWindow;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2Settings, ICoreWebView2Settings3,
    };
    use windows_core::Interface;

    let Some(main_window): Option<WebviewWindow> = app.get_webview_window("main") else {
        return;
    };
    lock_fixed_window_style(&main_window);
    let _: tauri::Result<()> = main_window.with_webview(|webview: PlatformWebview| {
        // SAFETY: `webview.controller()` yields the live `ICoreWebView2Controller` on the main STA
        // thread. Disabling default context menus and browser accelerator keys prevents accidental
        // browser reloads (`F5`/`Ctrl+R`) and context-menu refresh/inspect popups.
        unsafe {
            if let Ok(core) = webview.controller().CoreWebView2() {
                let core_ref: ICoreWebView2 = core;
                if let Ok(settings) = core_ref.Settings() {
                    let settings_ref: ICoreWebView2Settings = settings;
                    let _ = settings_ref.SetAreDefaultContextMenusEnabled(false);
                    let _ = settings_ref.SetAreDevToolsEnabled(false);
                    if let Ok(settings3) = settings_ref.cast::<ICoreWebView2Settings3>() {
                        let _ = settings3.SetAreBrowserAcceleratorKeysEnabled(false);
                    }
                }
            }
        }
    });
}

#[cfg(not(windows))]
fn harden_windows_webview(app: &App) {
    if let Some(main_window) = app.get_webview_window("main") {
        let _: tauri::Result<()> = main_window.set_resizable(false);
        let _: tauri::Result<()> = main_window.set_maximizable(false);
    }
}

/// Launches the Tauri v2 desktop application for `DH_ServerLauncher_JA` with zero HTTP listening ports.
pub fn run_tauri_app() -> Result<(), String> {
    let app_state: GuiAppState = GuiAppState::initialize();

    let app: App = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .setup(|app_ref: &mut App| {
            harden_windows_webview(app_ref);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_initial_state,
            commands::sync_ui_draft,
            commands::save_config,
            commands::reset_default_config,
            commands::pick_server_executable,
            commands::execute_admin_console_action,
            commands::open_logs_folder,
            commands::clear_console_logs,
            commands::clear_server_logs,
            commands::launch_server,
            commands::update_next_round_plan,
            commands::restart_server_now,
            commands::cancel_countdown_for_reconfigure,
            commands::stop_server,
            commands::get_license_text,
            commands::window_minimize,
            commands::window_close,
        ])
        .build(tauri::generate_context!())
        .map_err(|err: tauri::Error| format!("Tauriアプリケーションの起動に失敗しました: {err}"))?;

    app.run(|app_handle: &AppHandle, event: RunEvent| {
        if matches!(event, RunEvent::Exit) {
            let state: tauri::State<'_, GuiAppState> = app_handle.state::<GuiAppState>();
            let cloned: GuiAppState = state.inner().clone();
            tauri::async_runtime::block_on(async move {
                shutdown_all_services(&cloned).await;
            });
        }
    });

    Ok(())
}
