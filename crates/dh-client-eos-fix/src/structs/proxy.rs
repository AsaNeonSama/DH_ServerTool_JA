use std::ffi::CStr;

/// Metadata for one forwarded `dsound.dll` export slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxyExportSpec {
    /// Zero-based index into `DH_DSOUND_TARGETS`.
    pub index: usize,
    /// Fixed Win32 export ordinal (`1..=12`).
    pub ordinal: u16,
    /// Export symbol name.
    pub name: &'static str,
    /// Null-terminated ASCII export name for `GetProcAddress` fallback.
    pub c_name: &'static CStr,
}
