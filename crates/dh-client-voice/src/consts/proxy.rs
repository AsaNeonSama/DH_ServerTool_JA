use crate::structs::proxy::ProxyExportSpec;

/// Exact number of exports exposed by Windows `System32\X3DAudio1_7.dll`.
pub const X3DAUDIO_EXPORT_COUNT: usize = 2;

/// Filename appended to `GetSystemDirectoryW()` to locate the real system library.
pub const REAL_X3DAUDIO_DLL_NAME: &str = "X3DAudio1_7.dll";

/// Initial UTF-16 buffer capacity for Win32 path queries (`GetModuleFileNameW`, `GetSystemDirectoryW`).
pub const WIN32_PATH_BUF_CAPACITY: usize = 512;

/// Maximum UTF-16 buffer capacity when growing for long Windows paths.
pub const WIN32_PATH_BUF_MAX: usize = 32_768;

/// Both `X3DAudio1_7.dll` exports with their exact fixed ordinals (`1..=2`).
pub const X3DAUDIO_EXPORTS: [ProxyExportSpec; X3DAUDIO_EXPORT_COUNT] = [
    ProxyExportSpec {
        index: 0,
        ordinal: 1,
        name: "X3DAudioCalculate",
        c_name: c"X3DAudioCalculate",
    },
    ProxyExportSpec {
        index: 1,
        ordinal: 2,
        name: "X3DAudioInitialize",
        c_name: c"X3DAudioInitialize",
    },
];
