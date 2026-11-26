use crate::structs::proxy::ProxyExportSpec;

/// Exact number of exports exposed by Windows `System32\dsound.dll`.
pub const DSOUND_EXPORT_COUNT: usize = 12;

/// Filename appended to `GetSystemDirectoryW()` to locate the real system library.
pub const REAL_DSOUND_DLL_NAME: &str = "dsound.dll";

/// HRESULT returned by the fallback stub if `System32\dsound.dll` cannot be resolved (`E_FAIL`).
pub const E_FAIL_HRESULT: i32 = -2_147_467_259;

/// Initial UTF-16 buffer capacity for Win32 path queries (`GetModuleFileNameW`, `GetSystemDirectoryW`).
pub const WIN32_PATH_BUF_CAPACITY: usize = 512;

/// Maximum UTF-16 buffer capacity when growing for long Windows paths.
pub const WIN32_PATH_BUF_MAX: usize = 32_768;

/// All 12 `dsound.dll` exports with their exact fixed ordinals (`1..=12`).
pub const DSOUND_EXPORTS: [ProxyExportSpec; DSOUND_EXPORT_COUNT] = [
    ProxyExportSpec {
        index: 0,
        ordinal: 1,
        name: "DirectSoundCreate",
        c_name: c"DirectSoundCreate",
    },
    ProxyExportSpec {
        index: 1,
        ordinal: 2,
        name: "DirectSoundEnumerateA",
        c_name: c"DirectSoundEnumerateA",
    },
    ProxyExportSpec {
        index: 2,
        ordinal: 3,
        name: "DirectSoundEnumerateW",
        c_name: c"DirectSoundEnumerateW",
    },
    ProxyExportSpec {
        index: 3,
        ordinal: 4,
        name: "DllCanUnloadNow",
        c_name: c"DllCanUnloadNow",
    },
    ProxyExportSpec {
        index: 4,
        ordinal: 5,
        name: "DllGetClassObject",
        c_name: c"DllGetClassObject",
    },
    ProxyExportSpec {
        index: 5,
        ordinal: 6,
        name: "DirectSoundCaptureCreate",
        c_name: c"DirectSoundCaptureCreate",
    },
    ProxyExportSpec {
        index: 6,
        ordinal: 7,
        name: "DirectSoundCaptureEnumerateA",
        c_name: c"DirectSoundCaptureEnumerateA",
    },
    ProxyExportSpec {
        index: 7,
        ordinal: 8,
        name: "DirectSoundCaptureEnumerateW",
        c_name: c"DirectSoundCaptureEnumerateW",
    },
    ProxyExportSpec {
        index: 8,
        ordinal: 9,
        name: "GetDeviceID",
        c_name: c"GetDeviceID",
    },
    ProxyExportSpec {
        index: 9,
        ordinal: 10,
        name: "DirectSoundFullDuplexCreate",
        c_name: c"DirectSoundFullDuplexCreate",
    },
    ProxyExportSpec {
        index: 10,
        ordinal: 11,
        name: "DirectSoundCreate8",
        c_name: c"DirectSoundCreate8",
    },
    ProxyExportSpec {
        index: 11,
        ordinal: 12,
        name: "DirectSoundCaptureCreate8",
        c_name: c"DirectSoundCaptureCreate8",
    },
];
