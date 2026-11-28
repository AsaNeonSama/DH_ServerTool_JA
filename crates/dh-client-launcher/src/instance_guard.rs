use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};

use dh_win32::{list_processes, ProcessInfo};

use crate::consts::app::KNOWN_GAME_EXE_NAMES;
use crate::structs::state::{AppInstanceGuard, MutexAcquireOutcome, NamedMutexGuard};

impl Drop for AppInstanceGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.handle != 0 && self.handle != -1 {
            use std::ffi::c_void;
            use windows::Win32::Foundation::{CloseHandle, HANDLE};

            let raw: HANDLE = HANDLE(self.handle as *mut c_void);
            // SAFETY: `AppInstanceGuard` uniquely owns this Win32 mutex handle returned by
            // `CreateMutexW` and closes it exactly once upon drop.
            unsafe {
                let _ = CloseHandle(raw);
            }
            self.handle = 0;
        }
    }
}

impl Drop for NamedMutexGuard {
    fn drop(&mut self) {
        if let Some(tx) = self.release_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.owner_thread.take() {
            let _ = handle.join();
        }
    }
}

/// Builds the ordered, case-insensitively deduplicated list of target game executable filenames
/// to monitor for single-instance enforcement (configured exe filename first, followed by known
/// Dread Hunger client executables).
#[must_use]
pub fn target_game_exe_names(configured_exe_path: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::with_capacity(KNOWN_GAME_EXE_NAMES.len() + 1);
    let trimmed: &str = configured_exe_path.trim();
    if !trimmed.is_empty() {
        if let Some(file_name) = Path::new(trimmed)
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
        {
            let clean: &str = file_name.trim();
            if !clean.is_empty() {
                names.push(clean.to_string());
            }
        }
    }
    for known in KNOWN_GAME_EXE_NAMES {
        if !names
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(known))
        {
            names.push((*known).to_string());
        }
    }
    names
}

/// Searches `processes` in `exe_names` priority order for a case-insensitive filename match,
/// skipping PID 0 and an optional `exclude_pid` (e.g. the launcher's own PID).
#[must_use]
pub fn find_matching_process_in(
    processes: &[ProcessInfo],
    exe_names: &[String],
    exclude_pid: Option<u32>,
) -> Option<ProcessInfo> {
    for target_name in exe_names {
        for proc_info in processes {
            if proc_info.pid == 0 || Some(proc_info.pid) == exclude_pid {
                continue;
            }
            if proc_info.name.eq_ignore_ascii_case(target_name) {
                return Some(proc_info.clone());
            }
        }
    }
    None
}

/// Returns `true` if `pid` is currently running (`STILL_ACTIVE`) or exists with restricted access
/// (`ERROR_ACCESS_DENIED`, e.g. elevated process), while excluding exited/zombie handles.
#[cfg(windows)]
#[must_use]
pub fn is_pid_running(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE, STILL_ACTIVE};
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    if pid == 0 {
        return false;
    }

    // SAFETY: `OpenProcess` with `PROCESS_QUERY_LIMITED_INFORMATION` is a read-only kernel query;
    // any returned handle is closed immediately before this function returns.
    let open_res: windows::core::Result<HANDLE> =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
    match open_res {
        Ok(handle) => {
            if handle.is_invalid() {
                return false;
            }
            let mut exit_code: u32 = 0;
            // SAFETY: `handle` is valid and `&raw mut exit_code` points to writable `u32` storage.
            let ok: bool = unsafe { GetExitCodeProcess(handle, &raw mut exit_code) }.is_ok();
            // SAFETY: `handle` was returned by `OpenProcess` above and is closed exactly once here.
            unsafe {
                let _ = CloseHandle(handle);
            }
            ok && exit_code == STILL_ACTIVE.0.cast_unsigned()
        }
        Err(err) => err.code() == ERROR_ACCESS_DENIED.to_hresult(),
    }
}

#[cfg(not(windows))]
#[must_use]
pub fn is_pid_running(pid: u32) -> bool {
    dh_win32::is_process_alive(pid)
}

/// Scans running OS processes via Toolhelp32 for an already-running Dread Hunger game instance
/// matching the configured executable or known Dread Hunger client filenames.
#[must_use]
pub fn detect_running_game_process(configured_exe_path: &str) -> Option<ProcessInfo> {
    let exe_names: Vec<String> = target_game_exe_names(configured_exe_path);
    let self_pid: u32 = std::process::id();
    let live_processes: Vec<ProcessInfo> = list_processes()
        .into_iter()
        .filter(|p: &ProcessInfo| p.pid != 0 && p.pid != self_pid)
        .filter(|p: &ProcessInfo| {
            exe_names
                .iter()
                .any(|name: &String| p.name.eq_ignore_ascii_case(name))
        })
        .filter(|p: &ProcessInfo| is_pid_running(p.pid))
        .collect();
    find_matching_process_in(&live_processes, &exe_names, Some(self_pid))
}

/// Attempts to acquire an OS-wide Windows Named Mutex (`CreateMutexW` + `WaitForSingleObject`)
/// within `timeout_ms` milliseconds on a dedicated owning OS thread.
///
/// Returns `Ok(Some(guard))` when acquired, `Ok(None)` if another launcher process currently holds
/// the mutex past `timeout_ms`, or `Err(msg)` on unexpected Win32 failure.
#[cfg(windows)]
pub fn try_acquire_named_mutex(
    mutex_name: &str,
    timeout_ms: u32,
) -> Result<Option<NamedMutexGuard>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{
        CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_EVENT, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};

    if mutex_name.is_empty() || mutex_name.contains('\0') {
        return Err("ミューテックス名が不正です".to_string());
    }

    let mut wide_name: Vec<u16> = mutex_name.encode_utf16().collect();
    wide_name.push(0);

    let (ready_tx, ready_rx): (
        SyncSender<MutexAcquireOutcome>,
        Receiver<MutexAcquireOutcome>,
    ) = mpsc::sync_channel(1);
    let (release_tx, release_rx): (SyncSender<()>, Receiver<()>) = mpsc::sync_channel(1);

    let owner_thread: JoinHandle<()> = thread::Builder::new()
        .name("dh-launch-mutex".to_string())
        .spawn(move || {
            // SAFETY: `wide_name` is a valid NUL-terminated UTF-16LE string owned by this thread;
            // `CreateMutexW` creates or opens the named kernel mutex without requesting initial ownership.
            let create_res: windows::core::Result<HANDLE> =
                unsafe { CreateMutexW(None, false, PCWSTR::from_raw(wide_name.as_ptr())) };
            let handle: HANDLE = match create_res {
                Ok(h) if !h.is_invalid() => h,
                Ok(_) => {
                    let _ =
                        ready_tx.send(Err("CreateMutexW が無効なハンドルを返しました".to_string()));
                    return;
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(format!("CreateMutexW に失敗しました: {err}")));
                    return;
                }
            };

            // SAFETY: `handle` is a valid Win32 mutex handle created/opened above on this thread.
            let wait_res: WAIT_EVENT = unsafe { WaitForSingleObject(handle, timeout_ms) };
            if wait_res == WAIT_OBJECT_0 || wait_res == WAIT_ABANDONED {
                let _ = ready_tx.send(Ok(true));
                let _ = release_rx.recv();
                // SAFETY: This exact OS thread acquired ownership via `WaitForSingleObject` above,
                // so `ReleaseMutex` is called on the owning thread before closing the handle.
                unsafe {
                    let _ = ReleaseMutex(handle);
                    let _ = CloseHandle(handle);
                }
            } else if wait_res == WAIT_TIMEOUT {
                let _ = ready_tx.send(Ok(false));
                // SAFETY: `handle` is valid and owned by this thread; we did not acquire the lock.
                unsafe {
                    let _ = CloseHandle(handle);
                }
            } else {
                let _ = ready_tx.send(Err(format!(
                    "WaitForSingleObject が失敗しました (code: {:#x})",
                    wait_res.0
                )));
                // SAFETY: `handle` is valid and must be closed on error.
                unsafe {
                    let _ = CloseHandle(handle);
                }
            }
        })
        .map_err(|err: std::io::Error| format!("起動ガードスレッドの作成に失敗しました: {err}"))?;

    let acquired: bool = match ready_rx.recv() {
        Ok(Ok(flag)) => flag,
        Ok(Err(err)) => {
            let _ = owner_thread.join();
            return Err(err);
        }
        Err(_) => {
            let _ = owner_thread.join();
            return Err("起動ガードスレッドが応答しませんでした".to_string());
        }
    };

    if acquired {
        Ok(Some(NamedMutexGuard {
            release_tx: Some(release_tx),
            owner_thread: Some(owner_thread),
        }))
    } else {
        drop(release_tx);
        let _ = owner_thread.join();
        Ok(None)
    }
}

#[cfg(not(windows))]
pub fn try_acquire_named_mutex(
    _mutex_name: &str,
    _timeout_ms: u32,
) -> Result<Option<NamedMutexGuard>, String> {
    Ok(Some(NamedMutexGuard {
        release_tx: None,
        owner_thread: None,
    }))
}

/// Acquires an OS-wide Windows Named Mutex (`CreateMutexW`) for single-instance enforcement of
/// the launcher application itself.
///
/// Returns `Ok(Some(guard))` when this process is the sole active instance, `Ok(None)` when
/// `ERROR_ALREADY_EXISTS` indicates another launcher instance is already running, or `Err(msg)`
/// on unexpected Win32 failure.
#[cfg(windows)]
pub fn try_acquire_app_instance(mutex_name: &str) -> Result<Option<AppInstanceGuard>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
    use windows::Win32::System::Threading::CreateMutexW;

    if mutex_name.is_empty() || mutex_name.contains('\0') {
        return Err("アプリインスタンスミューテックス名が不正です".to_string());
    }

    let mut wide_name: Vec<u16> = mutex_name.encode_utf16().collect();
    wide_name.push(0);

    // SAFETY: `wide_name` is a valid NUL-terminated UTF-16LE string alive across `CreateMutexW`.
    // Immediately inspecting `GetLastError()` checks whether an existing instance already owns
    // this named kernel mutex; on `ERROR_ALREADY_EXISTS` we close our duplicate handle immediately.
    let handle: HANDLE = unsafe { CreateMutexW(None, false, PCWSTR::from_raw(wide_name.as_ptr())) }
        .map_err(|err: windows::core::Error| format!("CreateMutexW に失敗しました: {err}"))?;
    if handle.is_invalid() {
        return Err("CreateMutexW が無効なハンドルを返しました".to_string());
    }

    // SAFETY: `GetLastError` reads the calling thread's Win32 last-error code set by `CreateMutexW`.
    let last_err: windows::Win32::Foundation::WIN32_ERROR = unsafe { GetLastError() };
    if last_err == ERROR_ALREADY_EXISTS {
        // SAFETY: `handle` is a valid duplicate handle opened above and is closed exactly once here.
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Ok(None);
    }

    Ok(Some(AppInstanceGuard {
        handle: handle.0 as isize,
    }))
}

#[cfg(not(windows))]
pub fn try_acquire_app_instance(_mutex_name: &str) -> Result<Option<AppInstanceGuard>, String> {
    Ok(Some(AppInstanceGuard { handle: 0 }))
}

/// Finds an already-running launcher window by exact title, restores it if minimized, and brings
/// it to the foreground. Returns `true` if a matching window was found.
#[cfg(windows)]
#[must_use]
pub fn focus_existing_window(window_title: &str) -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
    };

    if window_title.is_empty() || window_title.contains('\0') {
        return false;
    }

    let mut wide_title: Vec<u16> = window_title.encode_utf16().collect();
    wide_title.push(0);

    // SAFETY: `wide_title` is a valid NUL-terminated UTF-16LE string; `FindWindowW`, `IsIconic`,
    // `ShowWindow`, and `SetForegroundWindow` are standard Win32 user32 window-activation APIs.
    let Ok(hwnd): windows::core::Result<HWND> =
        (unsafe { FindWindowW(PCWSTR::null(), PCWSTR::from_raw(wide_title.as_ptr())) })
    else {
        return false;
    };
    if hwnd.is_invalid() {
        return false;
    }

    // SAFETY: `hwnd` is a valid top-level window handle returned by `FindWindowW`.
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        let _ = SetForegroundWindow(hwnd);
    }
    true
}

#[cfg(not(windows))]
#[must_use]
pub fn focus_existing_window(_window_title: &str) -> bool {
    false
}
