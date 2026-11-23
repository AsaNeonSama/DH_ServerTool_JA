/// Result of a UDP listener lookup via `GetExtendedUdpTable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdpLookupResult {
    /// Whether the Windows IP Helper UDP table was successfully queried.
    pub available: bool,
    /// Owning PID of the matching UDP socket, if found.
    pub pid: Option<u32>,
}
