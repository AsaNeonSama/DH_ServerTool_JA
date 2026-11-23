use std::ffi::c_void;
use std::ptr::copy_nonoverlapping;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::Console::{
    GetConsoleMode, GetStdHandle, SetConsoleCP, SetConsoleMode, SetConsoleOutputCP,
    SetConsoleTitleW, CONSOLE_MODE, ENABLE_EXTENDED_FLAGS, ENABLE_MOUSE_INPUT,
    ENABLE_QUICK_EDIT_MODE, ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
    ENABLE_WINDOW_INPUT, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};

use crate::consts::console::{
    CF_UNICODETEXT, CLIPBOARD_RETRY_ATTEMPTS, CLIPBOARD_RETRY_DELAY_MS, CP_UTF8,
};
use crate::error::{encode_wide_null, from_windows_err, last_error, Result, Win32Error};

/// Sets console input/output code pages to UTF-8 (65001), updates the window title,
/// and enables ANSI Virtual Terminal processing on `STD_OUTPUT_HANDLE`.
///
/// Failures are ignored because stdout/stdin may be redirected in non-TTY environments.
pub fn init_console(title: &str) {
    // SAFETY: Setting console code pages to UTF-8 (65001) is a safe Win32 call with no pointer arguments.
    unsafe {
        let _ = SetConsoleOutputCP(CP_UTF8);
        let _ = SetConsoleCP(CP_UTF8);
    }
    let _ = set_console_title(title);
    let _ = enable_virtual_terminal_output();
}

/// Sets the Windows console title using UTF-16LE (`SetConsoleTitleW`).
pub fn set_console_title(title: &str) -> Result<()> {
    let wide: Vec<u16> = encode_wide_null(title);
    // SAFETY: `wide` is NUL-terminated UTF-16LE and remains valid for the duration of `SetConsoleTitleW`.
    unsafe { SetConsoleTitleW(PCWSTR::from_raw(wide.as_ptr())) }
        .map_err(|e: windows::core::Error| from_windows_err("SetConsoleTitleW", &e))
}

/// Enables `ENABLE_VIRTUAL_TERMINAL_PROCESSING` on `STD_OUTPUT_HANDLE`.
pub fn enable_virtual_terminal_output() -> Result<()> {
    // SAFETY: `GetStdHandle` queries a standard handle identifier and does not transfer handle ownership.
    let h_out: HANDLE =
        unsafe { GetStdHandle(STD_OUTPUT_HANDLE) }.map_err(|e: windows::core::Error| {
            from_windows_err("GetStdHandle(STD_OUTPUT_HANDLE)", &e)
        })?;
    if h_out.is_invalid() {
        return Err(Win32Error::InvalidArgument(
            "STD_OUTPUT_HANDLE is invalid".to_string(),
        ));
    }
    let mut mode: CONSOLE_MODE = CONSOLE_MODE::default();
    // SAFETY: `h_out` is a valid console handle and `&raw mut mode` points to writable `CONSOLE_MODE` storage.
    unsafe { GetConsoleMode(h_out, &raw mut mode) }
        .map_err(|e: windows::core::Error| from_windows_err("GetConsoleMode", &e))?;
    let next: CONSOLE_MODE = CONSOLE_MODE(mode.0 | ENABLE_VIRTUAL_TERMINAL_PROCESSING.0);
    // SAFETY: `h_out` is a valid console handle and `next` contains valid `CONSOLE_MODE` flags.
    unsafe { SetConsoleMode(h_out, next) }
        .map_err(|e: windows::core::Error| from_windows_err("SetConsoleMode", &e))
}

/// Returns the current `STD_INPUT_HANDLE` console mode bits, or `None` if stdin is not a console.
#[must_use]
pub fn get_input_mode() -> Option<u32> {
    // SAFETY: `GetStdHandle` queries `STD_INPUT_HANDLE` without transferring ownership.
    let h_in: HANDLE = unsafe { GetStdHandle(STD_INPUT_HANDLE) }.ok()?;
    if h_in.is_invalid() {
        return None;
    }
    let mut mode: CONSOLE_MODE = CONSOLE_MODE::default();
    // SAFETY: `h_in` is non-invalid and `&raw mut mode` points to initialized writable storage.
    unsafe { GetConsoleMode(h_in, &raw mut mode) }.ok()?;
    Some(mode.0)
}

/// Restores a previously saved `STD_INPUT_HANDLE` console mode.
#[must_use]
pub fn set_input_mode(mode: u32) -> bool {
    // SAFETY: `GetStdHandle` queries `STD_INPUT_HANDLE` without transferring ownership.
    let Ok(h_in): std::result::Result<HANDLE, windows::core::Error> =
        (unsafe { GetStdHandle(STD_INPUT_HANDLE) })
    else {
        return false;
    };
    if h_in.is_invalid() {
        return false;
    }
    // SAFETY: `h_in` is non-invalid and `CONSOLE_MODE(mode)` passes caller-provided console flags.
    unsafe { SetConsoleMode(h_in, CONSOLE_MODE(mode)) }.is_ok()
}

/// Configures `STD_INPUT_HANDLE` for mouse/window/VT input while disabling Quick Edit mode
/// so clicking tabs or copy buttons never freezes console output.
#[must_use]
pub fn enable_tab_mouse_input() -> bool {
    let Some(current): Option<u32> = get_input_mode() else {
        return false;
    };
    let enable_mask: u32 = ENABLE_MOUSE_INPUT.0
        | ENABLE_EXTENDED_FLAGS.0
        | ENABLE_VIRTUAL_TERMINAL_INPUT.0
        | ENABLE_WINDOW_INPUT.0;
    let next: u32 = (current | enable_mask) & !ENABLE_QUICK_EDIT_MODE.0;
    set_input_mode(next)
}

fn open_clipboard_with_retry() -> Result<()> {
    for attempt in 0u8..CLIPBOARD_RETRY_ATTEMPTS {
        // SAFETY: Passing `HWND::default()` (NULL) associates clipboard ownership with the current task.
        match unsafe { OpenClipboard(HWND::default()) } {
            Ok(()) => return Ok(()),
            Err(e) => {
                if attempt + 1 == CLIPBOARD_RETRY_ATTEMPTS {
                    return Err(from_windows_err("OpenClipboard", &e));
                }
                std::thread::sleep(Duration::from_millis(CLIPBOARD_RETRY_DELAY_MS));
            }
        }
    }
    Err(last_error("OpenClipboard"))
}

/// Writes `text` (ASCII or Unicode/Japanese) to the Windows clipboard as `CF_UNICODETEXT`.
pub fn set_clipboard_text(text: &str) -> Result<()> {
    let wide: Vec<u16> = encode_wide_null(text);
    let byte_len: usize = wide.len() * std::mem::size_of::<u16>();

    // SAFETY: `GlobalAlloc(GMEM_MOVEABLE, byte_len)` allocates movable global memory owned by this function.
    let hmem: HGLOBAL = unsafe { GlobalAlloc(GMEM_MOVEABLE, byte_len) }
        .map_err(|e: windows::core::Error| from_windows_err("GlobalAlloc", &e))?;

    // SAFETY: `hmem` was just allocated by `GlobalAlloc` and is valid to lock.
    let locked: *mut c_void = unsafe { GlobalLock(hmem) };
    if locked.is_null() {
        let err: Win32Error = last_error("GlobalLock");
        // SAFETY: `hmem` has not been transferred to the clipboard, so freeing it prevents a leak.
        unsafe {
            let _ = GlobalFree(hmem);
        }
        return Err(err);
    }

    // SAFETY: `locked` points to `byte_len` writable bytes allocated by `GlobalAlloc`; `wide` has exact size `byte_len` and does not overlap.
    unsafe {
        copy_nonoverlapping(wide.as_ptr().cast::<u8>(), locked.cast::<u8>(), byte_len);
        let _ = GlobalUnlock(hmem);
    }

    if let Err(err) = open_clipboard_with_retry() {
        // SAFETY: Clipboard open failed, so `hmem` is still owned locally and must be freed.
        unsafe {
            let _ = GlobalFree(hmem);
        }
        return Err(err);
    }

    let set_result: Result<()> = (|| -> Result<()> {
        // SAFETY: Clipboard is open by this thread; emptying it is required before `SetClipboardData`.
        unsafe { EmptyClipboard() }
            .map_err(|e: windows::core::Error| from_windows_err("EmptyClipboard", &e))?;
        // SAFETY: Clipboard is open and `hmem` is a valid unlocked `GMEM_MOVEABLE` UTF-16LE buffer.
        unsafe { SetClipboardData(CF_UNICODETEXT, HANDLE(hmem.0)) }
            .map_err(|e: windows::core::Error| from_windows_err("SetClipboardData", &e))?;
        Ok(())
    })();

    // SAFETY: Balances the successful `OpenClipboard` call above.
    unsafe {
        let _ = CloseClipboard();
    }

    if let Err(err) = set_result {
        // SAFETY: Ownership only transfers to the system when `SetClipboardData` succeeds.
        unsafe {
            let _ = GlobalFree(hmem);
        }
        return Err(err);
    }

    Ok(())
}

/// Reads `CF_UNICODETEXT` from the Windows clipboard.
pub fn get_clipboard_text() -> Result<String> {
    open_clipboard_with_retry()?;

    let read_result: Result<String> = (|| -> Result<String> {
        // SAFETY: Clipboard is currently open by this thread.
        let handle: HANDLE = unsafe { GetClipboardData(CF_UNICODETEXT) }
            .map_err(|e: windows::core::Error| from_windows_err("GetClipboardData", &e))?;
        let hmem: HGLOBAL = HGLOBAL(handle.0);
        // SAFETY: `hmem` is the `CF_UNICODETEXT` global handle owned by the clipboard while open.
        let size_bytes: usize = unsafe { GlobalSize(hmem) };
        if size_bytes < 2 {
            return Ok(String::new());
        }
        // SAFETY: Locking the clipboard's `HGLOBAL` returns a readable pointer valid until `GlobalUnlock`.
        let locked: *const c_void = unsafe { GlobalLock(hmem) };
        if locked.is_null() {
            return Err(last_error("GlobalLock"));
        }
        let max_u16: usize = size_bytes / 2;
        // SAFETY: `locked` is non-null, valid for `size_bytes` bytes (2-byte aligned Windows global heap allocation), and remains locked while read.
        let slice: &[u16] = unsafe { std::slice::from_raw_parts(locked.cast::<u16>(), max_u16) };
        let nul_pos: usize = slice
            .iter()
            .position(|&c: &u16| c == 0)
            .unwrap_or(slice.len());
        let text: String = String::from_utf16_lossy(&slice[..nul_pos]);
        // SAFETY: Matches the successful `GlobalLock(hmem)` call above before closing the clipboard.
        unsafe {
            let _ = GlobalUnlock(hmem);
        }
        Ok(text)
    })();

    // SAFETY: Balances the successful `OpenClipboard` call above.
    unsafe {
        let _ = CloseClipboard();
    }

    read_result
}
