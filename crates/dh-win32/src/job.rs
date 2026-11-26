use std::ffi::c_void;
use std::mem::size_of;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

use crate::error::{from_windows_err, Result, Win32Error};
use crate::process::SpawnedProcess;

pub use crate::structs::job::JobObject;

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
        if handle.is_invalid() {
            return Err(Win32Error::InvalidArgument(
                "CreateJobObjectW returned an invalid handle".to_string(),
            ));
        }

        // Wrap immediately so `Drop` guarantees `CloseHandle` on any subsequent failure.
        let job: Self = Self { handle };

        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION =
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let info_len: u32 = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
            .map_err(|_: std::num::TryFromIntError| {
                Win32Error::InvalidArgument(
                    "JOBOBJECT_EXTENDED_LIMIT_INFORMATION size exceeds u32".to_string(),
                )
            })?;

        // SAFETY: `job.handle` is a valid Job Object and `&raw const info` points to an initialized
        // `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` of length `info_len`.
        unsafe {
            SetInformationJobObject(
                job.handle,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast::<c_void>(),
                info_len,
            )
        }
        .map_err(|err: windows::core::Error| from_windows_err("SetInformationJobObject", &err))?;

        Ok(job)
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
