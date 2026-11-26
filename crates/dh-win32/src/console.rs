use std::ffi::c_void;
use std::mem::{align_of, size_of};
use std::ptr::{copy_nonoverlapping, read_unaligned};
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::Console::{
    GetConsoleMode, GetStdHandle, SetConsoleCP, SetConsoleMode, SetConsoleOutputCP,
    SetConsoleTitleW, CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_INPUT_HANDLE,
    STD_OUTPUT_HANDLE,
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
use crate::structs::console::{ClipboardGuard, GlobalAllocGuard, GlobalLockGuard};

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: `ClipboardGuard` is only constructed after a successful `OpenClipboard` on this thread.
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

impl GlobalAllocGuard {
    fn alloc_moveable(byte_len: usize) -> Result<Self> {
        // SAFETY: `GlobalAlloc(GMEM_MOVEABLE, byte_len)` allocates movable global memory owned by this guard.
        let handle: HGLOBAL = unsafe { GlobalAlloc(GMEM_MOVEABLE, byte_len) }
            .map_err(|e: windows::core::Error| from_windows_err("GlobalAlloc", &e))?;
        Ok(Self {
            handle,
            transferred: false,
        })
    }

    fn lock(&self) -> Result<GlobalLockGuard> {
        GlobalLockGuard::lock(self.handle)
    }

    fn mark_transferred(&mut self) {
        self.transferred = true;
    }
}

impl Drop for GlobalAllocGuard {
    fn drop(&mut self) {
        if !self.transferred && !self.handle.0.is_null() {
            // SAFETY: Ownership was not transferred to the clipboard, so freeing `self.handle` prevents a leak.
            unsafe {
                let _ = GlobalFree(self.handle);
            }
            self.handle = HGLOBAL::default();
        }
    }
}

impl GlobalLockGuard {
    fn lock(handle: HGLOBAL) -> Result<Self> {
        // SAFETY: `handle` is a valid `HGLOBAL` handle while locked by this guard.
        let ptr: *mut c_void = unsafe { GlobalLock(handle) };
        if ptr.is_null() {
            return Err(last_error("GlobalLock"));
        }
        Ok(Self { handle, ptr })
    }
}

impl Drop for GlobalLockGuard {
    fn drop(&mut self) {
        // SAFETY: Balances the successful `GlobalLock(self.handle)` call in `GlobalLockGuard::lock`.
        unsafe {
            let _ = GlobalUnlock(self.handle);
        }
    }
}

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

fn open_clipboard_with_retry() -> Result<ClipboardGuard> {
    for attempt in 0u8..CLIPBOARD_RETRY_ATTEMPTS {
        // SAFETY: Passing `HWND::default()` (NULL) associates clipboard ownership with the current task.
        match unsafe { OpenClipboard(HWND::default()) } {
            Ok(()) => return Ok(ClipboardGuard),
            Err(e) => {
                if attempt.saturating_add(1) >= CLIPBOARD_RETRY_ATTEMPTS {
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
    let byte_len: usize = wide.len().checked_mul(size_of::<u16>()).ok_or_else(|| {
        Win32Error::InvalidArgument("clipboard text UTF-16 byte length overflow".to_string())
    })?;

    let mut alloc_guard: GlobalAllocGuard = GlobalAllocGuard::alloc_moveable(byte_len)?;
    {
        let lock_guard: GlobalLockGuard = alloc_guard.lock()?;
        // SAFETY: `lock_guard.ptr` points to `byte_len` writable bytes allocated by `GlobalAlloc`;
        // `wide` has exact size `byte_len` and does not overlap.
        unsafe {
            copy_nonoverlapping(
                wide.as_ptr().cast::<u8>(),
                lock_guard.ptr.cast::<u8>(),
                byte_len,
            );
        }
    }

    let _clipboard_guard: ClipboardGuard = open_clipboard_with_retry()?;

    // SAFETY: Clipboard is open by this thread; emptying it is required before `SetClipboardData`.
    unsafe { EmptyClipboard() }
        .map_err(|e: windows::core::Error| from_windows_err("EmptyClipboard", &e))?;
    // SAFETY: Clipboard is open and `alloc_guard.handle` is a valid unlocked `GMEM_MOVEABLE` UTF-16LE buffer.
    unsafe { SetClipboardData(CF_UNICODETEXT, HANDLE(alloc_guard.handle.0)) }
        .map_err(|e: windows::core::Error| from_windows_err("SetClipboardData", &e))?;
    // Ownership transfers to the system only after `SetClipboardData` succeeds.
    alloc_guard.mark_transferred();

    Ok(())
}

/// Reads `CF_UNICODETEXT` from the Windows clipboard.
pub fn get_clipboard_text() -> Result<String> {
    let _clipboard_guard: ClipboardGuard = open_clipboard_with_retry()?;

    // SAFETY: Clipboard is currently open by this thread.
    let handle: HANDLE = unsafe { GetClipboardData(CF_UNICODETEXT) }
        .map_err(|e: windows::core::Error| from_windows_err("GetClipboardData", &e))?;
    let hmem: HGLOBAL = HGLOBAL(handle.0);
    if hmem.0.is_null() {
        return Ok(String::new());
    }

    // SAFETY: `hmem` is the `CF_UNICODETEXT` global handle owned by the clipboard while open.
    let size_bytes: usize = unsafe { GlobalSize(hmem) };
    if size_bytes < size_of::<u16>() {
        return Ok(String::new());
    }

    let lock_guard: GlobalLockGuard = GlobalLockGuard::lock(hmem)?;
    let max_u16: usize = size_bytes / size_of::<u16>();

    let text: String = if (lock_guard.ptr as usize).is_multiple_of(align_of::<u16>()) {
        // SAFETY: `lock_guard.ptr` is non-null, `u16`-aligned, valid for `size_bytes >= max_u16 * 2` bytes,
        // and remains locked for the duration of this borrow.
        let slice: &[u16] =
            unsafe { std::slice::from_raw_parts(lock_guard.ptr.cast::<u16>(), max_u16) };
        let nul_pos: usize = slice
            .iter()
            .position(|&c: &u16| c == 0)
            .unwrap_or(slice.len());
        String::from_utf16_lossy(&slice[..nul_pos])
    } else {
        let mut units: Vec<u16> = Vec::with_capacity(max_u16);
        let base_ptr: *const u8 = lock_guard.ptr.cast::<u8>();
        for idx in 0..max_u16 {
            let offset: usize = idx * size_of::<u16>();
            // SAFETY: `offset + size_of::<u16>() <= size_bytes` guarantees the read is in bounds,
            // and `read_unaligned` handles non-2-byte-aligned addresses safely.
            let unit: u16 = unsafe { read_unaligned(base_ptr.add(offset).cast::<u16>()) };
            if unit == 0 {
                break;
            }
            units.push(unit);
        }
        String::from_utf16_lossy(&units)
    };

    Ok(text)
}
