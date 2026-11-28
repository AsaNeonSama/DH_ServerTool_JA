/// Broken-down local calendar and wall-clock time obtained from Win32 `GetLocalTime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTimeParts {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub millisecond: u16,
}
