use windows::Win32::Foundation::{GetLastError, WIN32_ERROR};

pub use crate::structs::error::{Result, Win32Error};

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
    let err: windows::core::Error = windows::core::Error::from(win_err.to_hresult());
    Win32Error::Api {
        api,
        code: win_err.0,
        message: err.message(),
    }
}

pub(crate) fn encode_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0u16)).collect()
}

pub(crate) fn encode_wide_null_no_interior_nul(s: &str, field: &'static str) -> Result<Vec<u16>> {
    if s.contains('\0') {
        return Err(Win32Error::InvalidArgument(format!(
            "{field} must not contain interior NUL characters"
        )));
    }
    Ok(encode_wide_null(s))
}
