use windows::Win32::Foundation::{GetLastError, WIN32_ERROR};

/// Error type returned by `dh-win32` operations.
#[derive(Debug, thiserror::Error)]
pub enum Win32Error {
    #[error("Win32 API '{api}' failed (code {code:#010x}): {message}")]
    Api {
        api: &'static str,
        code: u32,
        message: String,
    },

    #[error("NT API '{api}' failed with NTSTATUS {status:#010x}")]
    NtStatus { api: &'static str, status: i32 },

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("PE header parse error: {0}")]
    InvalidPe(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Win32Error>;

pub(crate) fn from_windows_err(api: &'static str, err: &windows::core::Error) -> Win32Error {
    Win32Error::Api {
        api,
        code: err.code().0.cast_unsigned(),
        message: err.message(),
    }
}

pub(crate) fn last_error(api: &'static str) -> Win32Error {
    // SAFETY: `GetLastError` reads the calling thread's last-error slot and has no preconditions.
    let win_err: WIN32_ERROR = unsafe { GetLastError() };
    let err: windows::core::Error = windows::core::Error::from_win32();
    Win32Error::Api {
        api,
        code: win_err.0,
        message: err.message(),
    }
}

pub(crate) fn encode_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0u16)).collect()
}
