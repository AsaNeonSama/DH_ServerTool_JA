use std::ffi::c_void;
use std::mem::size_of;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

use crate::error::{from_windows_err, Result, Win32Error};
use crate::process::SpawnedProcess;

/// RAII wrapper around an anonymous Win32 Job Object configured with
/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
///
/// Closing or dropping this `JobObject` (or exiting/crashing the parent process)
/// instructs the Windows kernel to immediately terminate all assigned processes.
#[derive(Debug)]
pub struct JobObject {
    handle: HANDLE,
}

// SAFETY: Win32 Job Object kernel handles are safe to transfer across threads.
unsafe impl Send for JobObject {}
// SAFETY: Win32 Job Object kernel handles can be queried/assigned concurrently from multiple threads.
unsafe impl Sync for JobObject {}

impl JobObject {
    /// Creates an anonymous Win32 Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
    pub fn new_kill_on_close() -> Result<Self> {
        // SAFETY: Passing `None` security attributes and `PCWSTR::null()` creates a new anonymous Job Object handle.
        let handle: HANDLE = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(|e: windows::core::Error| from_windows_err("CreateJobObjectW", &e))?;

        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION =
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let info_len: u32 = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
            .map_err(|_: std::num::TryFromIntError| {
                Win32Error::InvalidArgument(
                    "JOBOBJECT_EXTENDED_LIMIT_INFORMATION size exceeds u32".to_string(),
                )
            })?;

        // SAFETY: `handle` is a valid Job Object and `&raw const info` points to an initialized `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` of length `info_len`.
        let res: windows::core::Result<()> = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast::<c_void>(),
                info_len,
            )
        };

        if let Err(err) = res {
            // SAFETY: `handle` is owned here and no longer returned to caller on error.
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(from_windows_err("SetInformationJobObject", &err));
        }

        Ok(Self { handle })
    }

    /// Assigns a process handle (raw `HANDLE` cast to `isize`) to this Job Object.
    pub fn assign_raw_handle(&self, process_handle: isize) -> Result<()> {
        if process_handle == 0 || process_handle == -1 {
            return Err(Win32Error::InvalidArgument(
                "invalid process handle".to_string(),
            ));
        }
        // SAFETY: `self.handle` is a live Job Object and `process_handle` is a non-null/non-sentinel Win32 process handle.
        unsafe { AssignProcessToJobObject(self.handle, HANDLE(process_handle as *mut c_void)) }
            .map_err(|e: windows::core::Error| from_windows_err("AssignProcessToJobObject", &e))
    }

    /// Assigns a [`SpawnedProcess`] directly using its owned process handle.
    pub fn assign_process(&self, process: &SpawnedProcess) -> Result<()> {
        self.assign_raw_handle(process.process_handle_raw())
    }

    /// Opens a running process by PID and assigns it to this Job Object.
    pub fn assign_pid(&self, pid: u32) -> Result<()> {
        if pid == 0 {
            return Err(Win32Error::InvalidArgument("pid must be > 0".to_string()));
        }
        // SAFETY: `OpenProcess` with `PROCESS_SET_QUOTA | PROCESS_TERMINATE` opens a handle to `pid` (closed below).
        let h_proc: HANDLE =
            unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid) }
                .map_err(|e: windows::core::Error| from_windows_err("OpenProcess", &e))?;

        // SAFETY: Both `self.handle` and `h_proc` are valid kernel handles.
        let res: Result<()> = unsafe { AssignProcessToJobObject(self.handle, h_proc) }
            .map_err(|e: windows::core::Error| from_windows_err("AssignProcessToJobObject", &e));
        // SAFETY: Closes the temporary process handle acquired by `OpenProcess`.
        unsafe {
            let _ = CloseHandle(h_proc);
        }
        res
    }

    /// Explicitly closes the Job Object handle, terminating all assigned processes.
    pub fn close(self) {
        drop(self);
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        if !self.handle.is_invalid() {
            // SAFETY: `self.handle` is uniquely owned by this `JobObject` and has not yet been closed.
            unsafe {
                let _ = CloseHandle(self.handle);
            }
            self.handle = HANDLE::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{is_process_alive, spawn_suspended_console};
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[test]
    fn job_object_kills_assigned_process_on_drop() {
        let cmd = Path::new(r"C:\Windows\System32\cmd.exe");
        let args = vec!["/C".to_string(), "ping -n 30 127.0.0.1 >NUL".to_string()];
        let proc = spawn_suspended_console(cmd, &args, None).expect("spawn cmd.exe suspended");
        let pid = proc.pid();

        let job = JobObject::new_kill_on_close().expect("create job object");
        job.assign_process(&proc).expect("assign process to job");
        proc.resume().expect("resume process");
        assert!(is_process_alive(pid));

        // Dropping the JobObject must immediately kill the assigned child process.
        drop(job);

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if !is_process_alive(pid) {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let _ = proc.terminate(1);
        panic!("process {pid} remained alive after JobObject drop");
    }
}
