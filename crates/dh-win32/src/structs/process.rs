use std::ffi::c_void;

use windows::Win32::Foundation::HANDLE;

/// Process entry returned by Toolhelp32 process enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
}

/// Entry-point metadata resolved from a remote x64 process's PEB and PE headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemotePeEntry {
    pub peb_address: u64,
    pub image_base: u64,
    pub entry_point_rva: u32,
    pub oep: u64,
}

/// Options controlling Win32 process creation flags and child environment for
/// [`crate::process::spawn_suspended`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnOptions {
    /// When `true`, passes `CREATE_NEW_CONSOLE` so console executables (e.g. UE4 dedicated server
    /// with `-log`) receive a fresh detached console window.
    /// When `false`, spawns with `CREATE_SUSPENDED` only so GUI applications (e.g.
    /// `DreadHunger-Win64-Shipping.exe`) launch without popping up a blank console window.
    pub create_console: bool,
    /// Optional per-child environment variable overrides (`(key, value)`) merged case-insensitively
    /// over the inherited environment via `CREATE_UNICODE_ENVIRONMENT` without mutating the parent.
    pub env_overrides: Vec<(String, String)>,
}

/// RAII handle owner for a child process spawned via `spawn_suspended`.
#[derive(Debug)]
pub struct SpawnedProcess {
    pub(crate) process_handle: HANDLE,
    pub(crate) thread_handle: HANDLE,
    pub(crate) pid: u32,
}

/// Internal RAII owner for temporary Win32 kernel handles closed via `CloseHandle`.
pub(crate) struct OwnedHandle(pub(crate) HANDLE);

#[repr(C)]
#[derive(Default)]
pub struct ProcessBasicInformation {
    pub exit_status: i32,
    pub(crate) _pad0: u32,
    pub peb_base_address: u64,
    pub affinity_mask: usize,
    pub base_priority: i32,
    pub(crate) _pad1: u32,
    pub unique_process_id: usize,
    pub inherited_from_unique_process_id: usize,
}

pub(crate) type NtQueryInformationProcessFn = unsafe extern "system" fn(
    process_handle: HANDLE,
    process_information_class: u32,
    process_information: *mut c_void,
    process_information_length: u32,
    return_length: *mut u32,
) -> i32;
