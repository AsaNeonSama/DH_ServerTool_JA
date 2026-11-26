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
}

pub type Result<T> = std::result::Result<T, Win32Error>;
