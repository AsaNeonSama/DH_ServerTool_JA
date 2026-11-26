use windows::Win32::Foundation::HANDLE;

/// RAII wrapper around an anonymous Win32 Job Object configured with
/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
///
/// Closing or dropping this `JobObject` (or exiting/crashing the parent process)
/// instructs the Windows kernel to immediately terminate all assigned processes.
#[derive(Debug)]
pub struct JobObject {
    pub(crate) handle: HANDLE,
}
