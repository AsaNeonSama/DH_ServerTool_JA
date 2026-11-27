use std::ffi::CStr;

/// Metadata for a single proxied `System32\X3DAudio1_7.dll` export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProxyExportSpec {
    /// Zero-based index into the lazy target table.
    pub index: usize,
    /// Fixed PE export ordinal in Windows `X3DAudio1_7.dll`.
    pub ordinal: u16,
    /// Export name for diagnostics and unit tests.
    pub name: &'static str,
    /// NUL-terminated ASCII name for `GetProcAddress` fallback lookup.
    pub c_name: &'static CStr,
}
