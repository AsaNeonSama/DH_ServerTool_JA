use std::borrow::Cow;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::path::Path;

use windows::core::{s, w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HMODULE, INVALID_HANDLE_VALUE, STILL_ACTIVE, WAIT_EVENT, WAIT_OBJECT_0,
    WAIT_TIMEOUT,
};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, OpenProcess, ResumeThread, TerminateProcess,
    WaitForSingleObject, CREATE_NEW_CONSOLE, CREATE_SUSPENDED, PROCESS_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, STARTUPINFOW,
};

use crate::error::{encode_wide_null, from_windows_err, Result, Win32Error};

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

/// RAII handle owner for a child process spawned via [`spawn_suspended_console`].
#[derive(Debug)]
pub struct SpawnedProcess {
    process_handle: HANDLE,
    thread_handle: HANDLE,
    pid: u32,
    thread_id: u32,
}

// SAFETY: Win32 process and thread handles are safe to move between threads.
unsafe impl Send for SpawnedProcess {}

impl SpawnedProcess {
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.pid
    }

    #[must_use]
    pub fn thread_id(&self) -> u32 {
        self.thread_id
    }

    /// Raw process `HANDLE` value as `isize`.
    #[must_use]
    pub fn process_handle_raw(&self) -> isize {
        self.process_handle.0 as isize
    }

    /// Raw primary thread `HANDLE` value as `isize`.
    #[must_use]
    pub fn thread_handle_raw(&self) -> isize {
        self.thread_handle.0 as isize
    }

    /// Resumes the primary thread of the spawned process, returning the previous suspend count.
    pub fn resume(&self) -> Result<u32> {
        // SAFETY: `self.thread_handle` is a valid primary thread handle owned by `SpawnedProcess`.
        let prev: u32 = unsafe { ResumeThread(self.thread_handle) };
        if prev == u32::MAX {
            return Err(crate::error::last_error("ResumeThread"));
        }
        Ok(prev)
    }

    /// Force-terminates the process via `TerminateProcess`.
    pub fn terminate(&self, exit_code: u32) -> Result<()> {
        // SAFETY: `self.process_handle` is a valid process handle owned by `SpawnedProcess`.
        unsafe { TerminateProcess(self.process_handle, exit_code) }
            .map_err(|e: windows::core::Error| from_windows_err("TerminateProcess", &e))
    }

    /// Checks whether the process is still running (`STILL_ACTIVE`).
    #[must_use]
    pub fn is_alive(&self) -> bool {
        let mut exit_code: u32 = 0;
        // SAFETY: `self.process_handle` is valid and `&raw mut exit_code` points to writable `u32` storage.
        let ok: bool =
            unsafe { GetExitCodeProcess(self.process_handle, &raw mut exit_code) }.is_ok();
        ok && exit_code == STILL_ACTIVE.0.cast_unsigned()
    }

    /// Waits up to `timeout_ms` milliseconds for the process to exit.
    /// Returns `Ok(Some(exit_code))` if exited, or `Ok(None)` on timeout.
    pub fn wait_exit(&self, timeout_ms: u32) -> Result<Option<u32>> {
        // SAFETY: `self.process_handle` is a valid waitable process handle owned by `SpawnedProcess`.
        let wait: WAIT_EVENT = unsafe { WaitForSingleObject(self.process_handle, timeout_ms) };
        if wait == WAIT_OBJECT_0 {
            let mut code: u32 = 0;
            // SAFETY: `self.process_handle` is valid and `&raw mut code` points to writable `u32` storage.
            unsafe { GetExitCodeProcess(self.process_handle, &raw mut code) }
                .map_err(|e: windows::core::Error| from_windows_err("GetExitCodeProcess", &e))?;
            Ok(Some(code))
        } else if wait == WAIT_TIMEOUT {
            Ok(None)
        } else {
            Err(crate::error::last_error("WaitForSingleObject"))
        }
    }
}

impl Drop for SpawnedProcess {
    fn drop(&mut self) {
        if !self.thread_handle.is_invalid() {
            // SAFETY: `self.thread_handle` is uniquely owned and has not yet been closed.
            unsafe {
                let _ = CloseHandle(self.thread_handle);
            }
            self.thread_handle = HANDLE::default();
        }
        if !self.process_handle.is_invalid() {
            // SAFETY: `self.process_handle` is uniquely owned and has not yet been closed.
            unsafe {
                let _ = CloseHandle(self.process_handle);
            }
            self.process_handle = HANDLE::default();
        }
    }
}

fn quote_windows_arg(arg: &str, force_quote: bool) -> String {
    let needs_quote: bool =
        force_quote || arg.is_empty() || arg.contains([' ', '\t', '"', '\n', '\r']);
    if !needs_quote {
        return arg.to_string();
    }

    let mut out: String = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes: usize = 0;
    for ch in arg.chars() {
        if ch == '\\' {
            backslashes += 1;
        } else if ch == '"' {
            for _ in 0..=(backslashes * 2) {
                out.push('\\');
            }
            out.push('"');
            backslashes = 0;
        } else {
            for _ in 0..backslashes {
                out.push('\\');
            }
            backslashes = 0;
            out.push(ch);
        }
    }
    for _ in 0..(backslashes * 2) {
        out.push('\\');
    }
    out.push('"');
    out
}

/// Builds a properly escaped Windows command line string for `CreateProcessW`.
#[must_use]
pub fn build_command_line(exe_path: &Path, args: &[String]) -> String {
    let exe_str: Cow<'_, str> = exe_path.to_string_lossy();
    let mut cmd: String = quote_windows_arg(&exe_str, true);
    for arg in args {
        cmd.push(' ');
        cmd.push_str(&quote_windows_arg(arg, false));
    }
    cmd
}

/// Spawns `exe_path` in suspended state with a fresh detached console window
/// (`CREATE_SUSPENDED | CREATE_NEW_CONSOLE`, `bInheritHandles = FALSE`, and no
/// `STARTF_USESTDHANDLES` so UE4 `-log` binds its own console output cleanly).
pub fn spawn_suspended_console(
    exe_path: &Path,
    args: &[String],
    cwd: Option<&Path>,
) -> Result<SpawnedProcess> {
    let exe_str: Cow<'_, str> = exe_path.to_string_lossy();
    if exe_str.is_empty() {
        return Err(Win32Error::InvalidArgument(
            "exe_path must not be empty".to_string(),
        ));
    }

    let app_wide: Vec<u16> = encode_wide_null(&exe_str);
    let mut cmd_wide: Vec<u16> = encode_wide_null(&build_command_line(exe_path, args));
    let work_dir_wide: Option<Vec<u16>> =
        cwd.map(|p: &Path| encode_wide_null(&p.to_string_lossy()));

    // SAFETY: `STARTUPINFOW` is a plain-old-data Win32 C struct valid when zero-initialized with `cb` set.
    let mut si: STARTUPINFOW = unsafe { zeroed() };
    si.cb = u32::try_from(size_of::<STARTUPINFOW>()).map_err(|_: std::num::TryFromIntError| {
        Win32Error::InvalidArgument("STARTUPINFOW size exceeds u32".to_string())
    })?;
    // Explicitly leave si.dwFlags == 0 (no STARTF_USESTDHANDLES).

    // SAFETY: `PROCESS_INFORMATION` is a plain-old-data Win32 output struct valid when zero-initialized.
    let mut pi: PROCESS_INFORMATION = unsafe { zeroed() };

    // SAFETY: `app_wide`, `cmd_wide`, and optional `work_dir_wide` are NUL-terminated UTF-16LE buffers
    // living across `CreateProcessW`; `&raw const si` and `&raw mut pi` point to valid initialized structs.
    unsafe {
        CreateProcessW(
            PCWSTR::from_raw(app_wide.as_ptr()),
            PWSTR::from_raw(cmd_wide.as_mut_ptr()),
            None,
            None,
            false,
            CREATE_SUSPENDED | CREATE_NEW_CONSOLE,
            None,
            work_dir_wide
                .as_ref()
                .map_or(PCWSTR::null(), |buf: &Vec<u16>| {
                    PCWSTR::from_raw(buf.as_ptr())
                }),
            &raw const si,
            &raw mut pi,
        )
    }
    .map_err(|e: windows::core::Error| from_windows_err("CreateProcessW", &e))?;

    Ok(SpawnedProcess {
        process_handle: pi.hProcess,
        thread_handle: pi.hThread,
        pid: pi.dwProcessId,
        thread_id: pi.dwThreadId,
    })
}

/// Returns `true` if a process with `pid` exists and has not exited (`STILL_ACTIVE`).
#[must_use]
pub fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: `OpenProcess` with `PROCESS_QUERY_LIMITED_INFORMATION` opens a read-only handle (closed below).
    let Ok(handle): std::result::Result<HANDLE, windows::core::Error> =
        (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return false;
    };
    let mut exit_code: u32 = 0;
    // SAFETY: `handle` is valid and `&raw mut exit_code` points to writable `u32` storage.
    let alive: bool = unsafe { GetExitCodeProcess(handle, &raw mut exit_code) }.is_ok()
        && exit_code == STILL_ACTIVE.0.cast_unsigned();
    // SAFETY: Closes the handle opened by `OpenProcess` above.
    unsafe {
        let _ = CloseHandle(handle);
    }
    alive
}

/// Terminates the process identified by `pid`, returning `true` on success.
#[must_use]
pub fn terminate_process(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: `OpenProcess` with `PROCESS_TERMINATE` opens a handle for termination (closed below).
    let Ok(handle): std::result::Result<HANDLE, windows::core::Error> =
        (unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) })
    else {
        return false;
    };
    // SAFETY: `handle` is a valid process handle opened with `PROCESS_TERMINATE`.
    let ok: bool = unsafe { TerminateProcess(handle, 0) }.is_ok();
    // SAFETY: Closes the handle opened by `OpenProcess` above.
    unsafe {
        let _ = CloseHandle(handle);
    }
    ok
}

/// Enumerates all running processes via `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)`.
#[must_use]
pub fn list_processes() -> Vec<ProcessInfo> {
    let mut results: Vec<ProcessInfo> = Vec::new();
    // SAFETY: `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)` takes a system process snapshot (closed below).
    let Ok(snap): std::result::Result<HANDLE, windows::core::Error> =
        (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) })
    else {
        return results;
    };
    if snap == INVALID_HANDLE_VALUE || snap.is_invalid() {
        return results;
    }

    let Ok(entry_size): std::result::Result<u32, std::num::TryFromIntError> =
        u32::try_from(size_of::<PROCESSENTRY32W>())
    else {
        // SAFETY: Closes the snapshot handle before returning.
        unsafe {
            let _ = CloseHandle(snap);
        }
        return results;
    };

    // SAFETY: `PROCESSENTRY32W` is a plain-old-data Win32 struct valid when zero-initialized with `dwSize` set.
    let mut entry: PROCESSENTRY32W = unsafe { zeroed() };
    entry.dwSize = entry_size;

    // SAFETY: `snap` is a valid Toolhelp32 snapshot handle and `&raw mut entry` points to a properly sized `PROCESSENTRY32W`.
    if unsafe { Process32FirstW(snap, &raw mut entry) }.is_ok() {
        loop {
            let nul_pos: usize = entry
                .szExeFile
                .iter()
                .position(|&c: &u16| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name: String = String::from_utf16_lossy(&entry.szExeFile[..nul_pos]);
            results.push(ProcessInfo {
                pid: entry.th32ProcessID,
                parent_pid: entry.th32ParentProcessID,
                name,
            });

            entry.dwSize = entry_size;
            // SAFETY: `snap` is valid and `&raw mut entry` points to a properly sized `PROCESSENTRY32W`.
            if unsafe { Process32NextW(snap, &raw mut entry) }.is_err() {
                break;
            }
        }
    }

    // SAFETY: Closes the snapshot handle opened by `CreateToolhelp32Snapshot`.
    unsafe {
        let _ = CloseHandle(snap);
    }
    results
}

/// Finds running processes whose executable filename matches `exe_name` case-insensitively.
#[must_use]
pub fn find_processes_by_name(exe_name: &str) -> Vec<ProcessInfo> {
    let target: String = exe_name.to_ascii_lowercase();
    list_processes()
        .into_iter()
        .filter(|p: &ProcessInfo| p.name.to_ascii_lowercase() == target)
        .collect()
}

#[repr(C)]
#[derive(Default)]
struct ProcessBasicInformation {
    exit_status: i32,
    _pad0: u32,
    peb_base_address: u64,
    affinity_mask: usize,
    base_priority: i32,
    _pad1: u32,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
}

type NtQueryInformationProcessFn = unsafe extern "system" fn(
    process_handle: HANDLE,
    process_information_class: u32,
    process_information: *mut c_void,
    process_information_length: u32,
    return_length: *mut u32,
) -> i32;

fn read_exact_remote(handle: HANDLE, addr: u64, buf: &mut [u8]) -> Result<()> {
    let mut bytes_read: usize = 0;
    // SAFETY: `handle` is a valid remote process handle and `buf` provides `buf.len()` writable bytes.
    unsafe {
        ReadProcessMemory(
            handle,
            addr as *const c_void,
            buf.as_mut_ptr().cast::<c_void>(),
            buf.len(),
            Some(&raw mut bytes_read),
        )
    }
    .map_err(|e: windows::core::Error| from_windows_err("ReadProcessMemory", &e))?;

    if bytes_read != buf.len() {
        return Err(Win32Error::InvalidPe(format!(
            "short ReadProcessMemory at {addr:#x}: expected {} bytes, got {bytes_read}",
            buf.len()
        )));
    }
    Ok(())
}

/// Reads the remote x64 process's PEB (`NtQueryInformationProcess`), extracts
/// `ImageBaseAddress` at `PEB + 0x10`, parses the PE32+ headers, and returns
/// the Original Entry Point (`OEP = ImageBase + AddressOfEntryPoint`).
pub fn read_remote_peb_and_oep(process_handle: isize) -> Result<RemotePeEntry> {
    if process_handle == 0 || process_handle == -1 {
        return Err(Win32Error::InvalidArgument(
            "invalid process handle".to_string(),
        ));
    }
    let handle: HANDLE = HANDLE(process_handle as *mut c_void);

    // SAFETY: `ntdll.dll` is permanently loaded in every Win32 process; `w!("ntdll.dll")` is a static NUL-terminated wide string.
    let ntdll: HMODULE = unsafe { GetModuleHandleW(w!("ntdll.dll")) }
        .map_err(|e: windows::core::Error| from_windows_err("GetModuleHandleW(ntdll.dll)", &e))?;
    // SAFETY: `ntdll` is a valid module handle and `s!("NtQueryInformationProcess")` is a static NUL-terminated ASCII string.
    let proc_addr: unsafe extern "system" fn() -> isize =
        unsafe { GetProcAddress(ntdll, s!("NtQueryInformationProcess")) }
            .ok_or_else(|| crate::error::last_error("GetProcAddress(NtQueryInformationProcess)"))?;
    // SAFETY: `NtQueryInformationProcess` in `ntdll.dll` has the exact `extern "system"` signature of `NtQueryInformationProcessFn`.
    let nt_query: NtQueryInformationProcessFn = unsafe {
        std::mem::transmute::<unsafe extern "system" fn() -> isize, NtQueryInformationProcessFn>(
            proc_addr,
        )
    };

    let mut pbi: ProcessBasicInformation = ProcessBasicInformation::default();
    let pbi_len: u32 = u32::try_from(size_of::<ProcessBasicInformation>()).map_err(
        |_: std::num::TryFromIntError| {
            Win32Error::InvalidPe("ProcessBasicInformation size exceeds u32".to_string())
        },
    )?;
    let mut ret_len: u32 = 0;
    // SAFETY: `handle` is non-null/non-sentinel, `&raw mut pbi` points to writable `ProcessBasicInformation` of size `pbi_len`, and `&raw mut ret_len` is writable.
    let status: i32 = unsafe {
        nt_query(
            handle,
            0, // ProcessBasicInformation
            (&raw mut pbi).cast::<c_void>(),
            pbi_len,
            &raw mut ret_len,
        )
    };
    if status < 0 {
        return Err(Win32Error::NtStatus {
            api: "NtQueryInformationProcess",
            status,
        });
    }
    if pbi.peb_base_address == 0 {
        return Err(Win32Error::InvalidPe(
            "remote PEB base address is null".to_string(),
        ));
    }

    // x64 PEB.ImageBaseAddress is at offset +0x10
    let mut image_base_bytes: [u8; 8] = [0u8; 8];
    read_exact_remote(handle, pbi.peb_base_address + 0x10, &mut image_base_bytes)?;
    let image_base: u64 = u64::from_le_bytes(image_base_bytes);
    if image_base == 0 {
        return Err(Win32Error::InvalidPe(
            "remote ImageBaseAddress is null".to_string(),
        ));
    }

    // Read IMAGE_DOS_HEADER (64 bytes)
    let mut dos_header: [u8; 64] = [0u8; 64];
    read_exact_remote(handle, image_base, &mut dos_header)?;
    let dos_magic: u16 = u16::from_le_bytes([dos_header[0], dos_header[1]]);
    if dos_magic != 0x5A4D {
        return Err(Win32Error::InvalidPe(format!(
            "invalid DOS signature {dos_magic:#06x} at {image_base:#x}"
        )));
    }
    let e_lfanew: u32 = u32::from_le_bytes([
        dos_header[0x3C],
        dos_header[0x3D],
        dos_header[0x3E],
        dos_header[0x3F],
    ]);
    if e_lfanew == 0 || e_lfanew > 0x0010_0000 {
        return Err(Win32Error::InvalidPe(format!(
            "unreasonable e_lfanew: {e_lfanew:#x}"
        )));
    }

    // Read PE signature (4) + IMAGE_FILE_HEADER (20) + first 24 bytes of IMAGE_OPTIONAL_HEADER64
    let mut nt_header: [u8; 0x30] = [0u8; 0x30];
    read_exact_remote(handle, image_base + u64::from(e_lfanew), &mut nt_header)?;

    let pe_sig: u32 = u32::from_le_bytes([nt_header[0], nt_header[1], nt_header[2], nt_header[3]]);
    if pe_sig != 0x0000_4550 {
        return Err(Win32Error::InvalidPe(format!(
            "invalid PE signature {pe_sig:#010x}"
        )));
    }

    let machine: u16 = u16::from_le_bytes([nt_header[4], nt_header[5]]);
    if machine != 0x8664 {
        return Err(Win32Error::InvalidPe(format!(
            "target is not x86_64 PE (Machine={machine:#06x})"
        )));
    }

    let opt_magic: u16 = u16::from_le_bytes([nt_header[0x18], nt_header[0x19]]);
    if opt_magic != 0x020B {
        return Err(Win32Error::InvalidPe(format!(
            "target is not PE32+ (OptionalHeader.Magic={opt_magic:#06x})"
        )));
    }

    // AddressOfEntryPoint is at OptionalHeader + 0x10 == NT header + 0x28
    let entry_point_rva: u32 = u32::from_le_bytes([
        nt_header[0x28],
        nt_header[0x29],
        nt_header[0x2A],
        nt_header[0x2B],
    ]);
    if entry_point_rva == 0 {
        return Err(Win32Error::InvalidPe(
            "AddressOfEntryPoint is zero".to_string(),
        ));
    }

    Ok(RemotePeEntry {
        peb_address: pbi.peb_base_address,
        image_base,
        entry_point_rva,
        oep: image_base + u64::from(entry_point_rva),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pbi_struct_is_48_bytes_on_x64() {
        assert_eq!(size_of::<ProcessBasicInformation>(), 48);
        assert_eq!(
            std::mem::offset_of!(ProcessBasicInformation, peb_base_address),
            8
        );
    }

    #[test]
    fn command_line_quoting_handles_spaces_and_ue_args() {
        let exe = Path::new(r"C:\Program Files\Dread Hunger\Server.exe");
        let args = vec![
            "DreadHunger".to_string(),
            "Departure_Persistent?listen?Game=/Game/Test".to_string(),
            "-log".to_string(),
            "-port=7895".to_string(),
            "arg with spaces".to_string(),
        ];
        let cmd = build_command_line(exe, &args);
        assert_eq!(
            cmd,
            r#""C:\Program Files\Dread Hunger\Server.exe" DreadHunger Departure_Persistent?listen?Game=/Game/Test -log -port=7895 "arg with spaces""#
        );
    }

    #[test]
    fn current_process_is_alive_and_enumerable() {
        let my_pid = std::process::id();
        assert!(is_process_alive(my_pid));
        assert!(!is_process_alive(0));

        let procs = list_processes();
        let me = procs
            .iter()
            .find(|p| p.pid == my_pid)
            .expect("current test exe in snapshot");
        let by_name = find_processes_by_name(&me.name.to_ascii_uppercase());
        assert!(by_name.iter().any(|p| p.pid == my_pid));
    }

    #[test]
    fn spawn_suspended_reads_valid_peb_and_oep() {
        let cmd = Path::new(r"C:\Windows\System32\cmd.exe");
        let args = vec!["/C".to_string(), "exit 0".to_string()];
        let proc = spawn_suspended_console(cmd, &args, None).expect("spawn cmd.exe suspended");
        let pe = read_remote_peb_and_oep(proc.process_handle_raw()).expect("read PEB and OEP");

        assert_ne!(pe.peb_address, 0);
        assert_ne!(pe.image_base, 0);
        assert_ne!(pe.entry_point_rva, 0);
        assert_eq!(pe.oep, pe.image_base + u64::from(pe.entry_point_rva));

        proc.resume().expect("resume cmd.exe");
        let code = proc.wait_exit(5000).expect("wait cmd.exe");
        assert_eq!(code, Some(0));
    }
}
