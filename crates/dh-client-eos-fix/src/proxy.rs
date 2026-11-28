use std::sync::atomic::{AtomicUsize, Ordering};

use crate::consts::proxy::{DSOUND_EXPORT_COUNT, E_FAIL_HRESULT};
use crate::structs::patch::PatchReport;

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::sync::OnceLock;
#[cfg(windows)]
use windows::core::{BOOL, PCSTR, PCWSTR};
#[cfg(windows)]
use windows::Win32::Foundation::HMODULE;
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::{
    DisableThreadLibraryCalls, GetProcAddress, LoadLibraryW,
};
#[cfg(windows)]
use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
#[cfg(windows)]
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
#[cfg(windows)]
use windows::Win32::System::Threading::{InitOnceExecuteOnce, INIT_ONCE};

#[cfg(windows)]
use crate::consts::proxy::{
    DSOUND_EXPORTS, REAL_DSOUND_DLL_NAME, WIN32_PATH_BUF_CAPACITY, WIN32_PATH_BUF_MAX,
};
#[cfg(windows)]
use crate::patcher::apply_client_patches_if_target;

/// Resolved function pointers for the 12 real `System32\dsound.dll` exports.
///
/// Populated lazily outside `DllMain` via `InitOnceExecuteOnce` on first export call.
static DH_DSOUND_TARGETS: [AtomicUsize; DSOUND_EXPORT_COUNT] = [
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
];

#[cfg(windows)]
static mut REAL_DSOUND_INIT_ONCE: INIT_ONCE = INIT_ONCE {
    Ptr: std::ptr::null_mut(),
};

#[cfg(windows)]
static LAST_PATCH_REPORT: OnceLock<PatchReport> = OnceLock::new();

/// Returns the startup patch report captured during `DLL_PROCESS_ATTACH`, if initialized.
#[cfg(windows)]
#[must_use]
pub fn last_patch_report() -> Option<PatchReport> {
    LAST_PATCH_REPORT.get().copied()
}

/// Non-Windows stub for `last_patch_report`.
#[cfg(not(windows))]
#[must_use]
pub fn last_patch_report() -> Option<PatchReport> {
    None
}

/// Safe fallback stub returned if `System32\dsound.dll` or an individual export is unavailable.
extern "system" fn dh_missing_dsound_export() -> i32 {
    E_FAIL_HRESULT
}

/// Lazily loads `System32\dsound.dll` outside `DllMain` and returns the resolved address
/// for export `index` (`0..12`). Always returns a valid executable function pointer.
extern "C" fn dh_resolve_dsound_export(index: usize) -> usize {
    #[cfg(windows)]
    {
        ensure_real_dsound_loaded();
    }

    let fallback: usize = dh_missing_dsound_export as *const () as usize;
    let Some(slot): Option<&AtomicUsize> = DH_DSOUND_TARGETS.get(index) else {
        return fallback;
    };
    let resolved: usize = slot.load(Ordering::Acquire);
    if resolved == 0 {
        fallback
    } else {
        resolved
    }
}

/// DLL entry point invoked by the Windows loader.
///
/// Never calls `LoadLibraryW` (preventing Loader Lock deadlocks); only disables thread
/// notifications and applies verified in-memory patches when attached to
/// `DreadHunger-Win64-Shipping.exe`.
#[cfg(windows)]
#[no_mangle]
extern "system" fn DllMain(instance: HMODULE, call_reason: u32, _reserved: *mut c_void) -> BOOL {
    if call_reason == DLL_PROCESS_ATTACH {
        // SAFETY: `instance` is the valid HMODULE supplied by the Windows loader.
        let _disabled: Result<(), windows::core::Error> =
            unsafe { DisableThreadLibraryCalls(instance) };

        let report_result: Result<PatchReport, Box<dyn std::any::Any + Send>> =
            std::panic::catch_unwind(apply_client_patches_if_target);
        if let Ok(report) = report_result {
            let _stored: Result<(), PatchReport> = LAST_PATCH_REPORT.set(report);
        }
    }
    BOOL::from(true)
}

#[cfg(windows)]
fn ensure_real_dsound_loaded() {
    // SAFETY: `InitOnceExecuteOnce` synchronizes one-time lazy loading of `System32\dsound.dll`
    // across threads outside `DllMain`, using `REAL_DSOUND_INIT_ONCE` exclusively through this API.
    let _init_res: Result<(), windows::core::Error> = unsafe {
        InitOnceExecuteOnce(
            &raw mut REAL_DSOUND_INIT_ONCE,
            Some(init_real_dsound_callback),
            None,
            None,
        )
    };
}

#[cfg(windows)]
unsafe extern "system" fn init_real_dsound_callback(
    _init_once: *mut INIT_ONCE,
    _param: *mut c_void,
    _context: *mut *mut c_void,
) -> BOOL {
    let fallback: usize = dh_missing_dsound_export as *const () as usize;
    let Some(system_dll_wide): Option<Vec<u16>> = build_system_dsound_path_wide() else {
        store_fallback_targets(fallback);
        return BOOL::from(true);
    };

    // SAFETY: `system_dll_wide` is NUL-terminated UTF-16LE pointing to `<System32>\dsound.dll`.
    // Called lazily outside `DllMain` via `InitOnceExecuteOnce`.
    let Ok(real_module): Result<HMODULE, windows::core::Error> =
        (unsafe { LoadLibraryW(PCWSTR(system_dll_wide.as_ptr())) })
    else {
        store_fallback_targets(fallback);
        return BOOL::from(true);
    };

    for spec in &DSOUND_EXPORTS {
        let ordinal_ptr: PCSTR = PCSTR(usize::from(spec.ordinal) as *const u8);
        // SAFETY: `real_module` is a valid loaded HMODULE and `ordinal_ptr` uses MAKEINTRESOURCEA
        // convention (`HIWORD == 0`) for fixed `dsound.dll` ordinals `1..=12`.
        let by_ordinal: Option<unsafe extern "system" fn() -> isize> =
            unsafe { GetProcAddress(real_module, ordinal_ptr) };

        let resolved_fn: Option<unsafe extern "system" fn() -> isize> = by_ordinal.or_else(|| {
            let name_ptr: PCSTR = PCSTR(spec.c_name.as_ptr().cast::<u8>());
            // SAFETY: `spec.c_name` is a static NUL-terminated ASCII CStr.
            unsafe { GetProcAddress(real_module, name_ptr) }
        });

        let addr: usize = resolved_fn
            .map_or(fallback, |func: unsafe extern "system" fn() -> isize| {
                func as usize
            });
        DH_DSOUND_TARGETS[spec.index].store(addr, Ordering::Release);
    }

    BOOL::from(true)
}

#[cfg(windows)]
fn store_fallback_targets(fallback: usize) {
    for slot in &DH_DSOUND_TARGETS {
        slot.store(fallback, Ordering::Release);
    }
}

#[cfg(windows)]
fn build_system_dsound_path_wide() -> Option<Vec<u16>> {
    let mut capacity: usize = WIN32_PATH_BUF_CAPACITY;
    while capacity <= WIN32_PATH_BUF_MAX {
        let mut buf: Vec<u16> = vec![0u16; capacity];
        // SAFETY: `buf` is a valid writable UTF-16 slice passed to `GetSystemDirectoryW`.
        let written: u32 = unsafe { GetSystemDirectoryW(Some(&mut buf)) };
        if written == 0 {
            return None;
        }
        let written_usize: usize = usize::try_from(written).ok()?;
        if written_usize < capacity {
            let mut result: Vec<u16> = buf[..written_usize].to_vec();
            if result.last().copied() != Some(u16::from(b'\\')) {
                result.push(u16::from(b'\\'));
            }
            result.extend(REAL_DSOUND_DLL_NAME.encode_utf16());
            result.push(0u16);
            return Some(result);
        }
        capacity = written_usize.checked_add(1)?;
    }
    None
}

// Naked x86-64 tail-jump thunks for all 12 `dsound.dll` exports.
// Fast path jumps directly through `DH_DSOUND_TARGETS[index]` without touching the stack or any
// argument registers (`rcx`, `rdx`, `r8`, `r9`, `xmm0..xmm3`, stack args).
// Cold path preserves all volatile argument registers, invokes `dh_resolve_dsound_export(index)`,
// restores them, and tail-jumps to the resolved address.
#[cfg(all(windows, target_arch = "x86_64"))]
core::arch::global_asm!(
    ".globl dh_proxy_direct_sound_create",
    "dh_proxy_direct_sound_create:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 0]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    xor eax, eax",
    "    jmp dh_dsound_lazy_resolve_common",
    ".globl dh_proxy_direct_sound_enumerate_a",
    "dh_proxy_direct_sound_enumerate_a:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 8]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    mov eax, 1",
    "    jmp dh_dsound_lazy_resolve_common",
    ".globl dh_proxy_direct_sound_enumerate_w",
    "dh_proxy_direct_sound_enumerate_w:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 16]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    mov eax, 2",
    "    jmp dh_dsound_lazy_resolve_common",
    ".globl dh_proxy_dll_can_unload_now",
    "dh_proxy_dll_can_unload_now:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 24]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    mov eax, 3",
    "    jmp dh_dsound_lazy_resolve_common",
    ".globl dh_proxy_dll_get_class_object",
    "dh_proxy_dll_get_class_object:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 32]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    mov eax, 4",
    "    jmp dh_dsound_lazy_resolve_common",
    ".globl dh_proxy_direct_sound_capture_create",
    "dh_proxy_direct_sound_capture_create:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 40]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    mov eax, 5",
    "    jmp dh_dsound_lazy_resolve_common",
    ".globl dh_proxy_direct_sound_capture_enumerate_a",
    "dh_proxy_direct_sound_capture_enumerate_a:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 48]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    mov eax, 6",
    "    jmp dh_dsound_lazy_resolve_common",
    ".globl dh_proxy_direct_sound_capture_enumerate_w",
    "dh_proxy_direct_sound_capture_enumerate_w:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 56]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    mov eax, 7",
    "    jmp dh_dsound_lazy_resolve_common",
    ".globl dh_proxy_get_device_id",
    "dh_proxy_get_device_id:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 64]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    mov eax, 8",
    "    jmp dh_dsound_lazy_resolve_common",
    ".globl dh_proxy_direct_sound_full_duplex_create",
    "dh_proxy_direct_sound_full_duplex_create:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 72]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    mov eax, 9",
    "    jmp dh_dsound_lazy_resolve_common",
    ".globl dh_proxy_direct_sound_create8",
    "dh_proxy_direct_sound_create8:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 80]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    mov eax, 10",
    "    jmp dh_dsound_lazy_resolve_common",
    ".globl dh_proxy_direct_sound_capture_create8",
    "dh_proxy_direct_sound_capture_create8:",
    "    lea rax, [rip + {targets}]",
    "    mov rax, qword ptr [rax + 88]",
    "    test rax, rax",
    "    jz 2f",
    "    jmp rax",
    "2:",
    "    mov eax, 11",
    "    jmp dh_dsound_lazy_resolve_common",
    "dh_dsound_lazy_resolve_common:",
    "    push rcx",
    "    push rdx",
    "    push r8",
    "    push r9",
    "    sub rsp, 0x68",
    "    movups xmmword ptr [rsp + 0x20], xmm0",
    "    movups xmmword ptr [rsp + 0x30], xmm1",
    "    movups xmmword ptr [rsp + 0x40], xmm2",
    "    movups xmmword ptr [rsp + 0x50], xmm3",
    "    mov ecx, eax",
    "    call {resolver}",
    "    movups xmm0, xmmword ptr [rsp + 0x20]",
    "    movups xmm1, xmmword ptr [rsp + 0x30]",
    "    movups xmm2, xmmword ptr [rsp + 0x40]",
    "    movups xmm3, xmmword ptr [rsp + 0x50]",
    "    add rsp, 0x68",
    "    pop r9",
    "    pop r8",
    "    pop rdx",
    "    pop rcx",
    "    jmp rax",
    targets = sym DH_DSOUND_TARGETS,
    resolver = sym dh_resolve_dsound_export,
);
