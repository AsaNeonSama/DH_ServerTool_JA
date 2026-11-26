use std::ffi::c_void;

use windows::Win32::Foundation::HGLOBAL;

/// RAII guard closing the Windows clipboard on drop.
pub(crate) struct ClipboardGuard;

/// RAII guard freeing an unowned `HGLOBAL` allocation unless ownership was transferred.
pub(crate) struct GlobalAllocGuard {
    pub(crate) handle: HGLOBAL,
    pub(crate) transferred: bool,
}

/// RAII guard unlocking a locked `HGLOBAL` handle on drop.
pub(crate) struct GlobalLockGuard {
    pub(crate) handle: HGLOBAL,
    pub(crate) ptr: *mut c_void,
}
