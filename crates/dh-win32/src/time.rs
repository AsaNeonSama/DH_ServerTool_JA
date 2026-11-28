use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::System::SystemInformation::GetLocalTime;

pub use crate::structs::time::LocalTimeParts;

/// Returns the current local date and time via Win32 `GetLocalTime`.
#[must_use]
pub fn get_local_time() -> LocalTimeParts {
    // SAFETY: `GetLocalTime` writes a `SYSTEMTIME` value directly with no pointer preconditions.
    let st: SYSTEMTIME = unsafe { GetLocalTime() };
    LocalTimeParts {
        year: st.wYear,
        month: st.wMonth,
        day: st.wDay,
        hour: st.wHour,
        minute: st.wMinute,
        second: st.wSecond,
        millisecond: st.wMilliseconds,
    }
}
