use std::path::PathBuf;

/// Error type returned by `dh-gadget-injector` operations.
#[derive(Debug, thiserror::Error)]
pub enum InjectorError {
    #[error("Win32 error: {0}")]
    Win32(#[from] dh_win32::Win32Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("failed to decompress embedded Frida Gadget: {0}")]
    Decompress(String),

    #[error("no writable cache directory available")]
    NoWritableCacheDir,

    #[error("DLL path too long for remote stub ({len} UTF-16 code units, max {max}): {path}")]
    PathTooLong {
        path: PathBuf,
        len: usize,
        max: usize,
    },

    #[error("DLL to inject not found: {path}")]
    DllNotFound { path: PathBuf },

    #[error("too many DLLs requested for OEP injection ({count}, max {max})")]
    TooManyDlls { count: usize, max: usize },

    #[error("LoadLibraryW returned NULL in target process for DLL: {path}")]
    DllLoadFailed { path: PathBuf },

    #[error("remote injection failed: {0}")]
    Inject(String),

    #[error("MOD initialization failed in '{file_name}': {message}")]
    ModInit { file_name: String, message: String },

    #[error("timed out waiting for Frida Gadget INIT_OK after {timeout_secs}s ({detail})")]
    InitTimeout { timeout_secs: u64, detail: String },

    #[error(
        "target process (PID {pid}) exited prematurely before INIT_OK (exit code: {exit_code:?})"
    )]
    PrematureExit { pid: u32, exit_code: Option<u32> },

    #[error("injection cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, InjectorError>;
