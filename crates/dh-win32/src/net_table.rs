use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
    MIB_UDP6ROW_OWNER_PID, MIB_UDPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

const IP_TABLE_MIN_BUFFER_SIZE: usize = 4096;
const IP_TABLE_EXTRA_BUFFER_SIZE: usize = 2048;

/// Result of a UDP listener lookup via `GetExtendedUdpTable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdpLookupResult {
    /// Whether the Windows IP Helper UDP table was successfully queried.
    pub available: bool,
    /// Owning PID of the matching UDP socket, if found.
    pub pid: Option<u32>,
}

#[inline]
fn port_from_network_dword(raw_port: u32) -> u16 {
    let low_word: u16 = u16::try_from(raw_port & 0xFFFF).unwrap_or(0);
    u16::from_be(low_word)
}

fn fetch_udp_table(address_family: u32) -> Option<Vec<u8>> {
    let mut needed: u32 = 0;
    // SAFETY: Passing `None` buffer queries the required byte size into `&raw mut needed`.
    let status: u32 = unsafe {
        GetExtendedUdpTable(
            None,
            &raw mut needed,
            false,
            address_family,
            UDP_TABLE_OWNER_PID,
            0,
        )
    };
    if status != ERROR_INSUFFICIENT_BUFFER.0 && status != NO_ERROR.0 {
        return None;
    }

    let mut alloc_size: usize =
        (needed as usize).max(IP_TABLE_MIN_BUFFER_SIZE) + IP_TABLE_EXTRA_BUFFER_SIZE;
    let mut buf: Vec<u8> = vec![0u8; alloc_size];

    for _ in 0u8..4u8 {
        let mut size_arg: u32 = u32::try_from(alloc_size).ok()?;
        // SAFETY: `buf` has capacity and length `alloc_size == size_arg as usize`, writable for `GetExtendedUdpTable`.
        let status: u32 = unsafe {
            GetExtendedUdpTable(
                Some(buf.as_mut_ptr().cast::<c_void>()),
                &raw mut size_arg,
                false,
                address_family,
                UDP_TABLE_OWNER_PID,
                0,
            )
        };
        if status == NO_ERROR.0 {
            return Some(buf);
        }
        if status != ERROR_INSUFFICIENT_BUFFER.0 {
            return None;
        }
        alloc_size = (size_arg as usize).max(alloc_size) + IP_TABLE_EXTRA_BUFFER_SIZE;
        buf.resize(alloc_size, 0);
    }

    None
}

fn query_udp_v4(port: u16, expected_pid: Option<u32>) -> UdpLookupResult {
    let Some(buf): Option<Vec<u8>> = fetch_udp_table(u32::from(AF_INET.0)) else {
        return UdpLookupResult {
            available: false,
            pid: None,
        };
    };
    if buf.len() < 4 {
        return UdpLookupResult {
            available: false,
            pid: None,
        };
    }

    let count: usize = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let row_size: usize = size_of::<MIB_UDPROW_OWNER_PID>();
    for i in 0..count {
        let offset: usize = 4 + i * row_size;
        if offset + row_size > buf.len() {
            break;
        }
        // SAFETY: `offset + row_size <= buf.len()` guarantees `MIB_UDPROW_OWNER_PID` lies within `buf`, and `read_unaligned` handles arbitrary alignment.
        let row: MIB_UDPROW_OWNER_PID = unsafe {
            buf.as_ptr()
                .add(offset)
                .cast::<MIB_UDPROW_OWNER_PID>()
                .read_unaligned()
        };
        if port_from_network_dword(row.dwLocalPort) == port {
            let owner_pid: u32 = row.dwOwningPid;
            if expected_pid.is_none_or(|exp: u32| owner_pid == exp) {
                return UdpLookupResult {
                    available: true,
                    pid: Some(owner_pid),
                };
            }
        }
    }

    UdpLookupResult {
        available: true,
        pid: None,
    }
}

fn query_udp_v6(port: u16, expected_pid: Option<u32>) -> UdpLookupResult {
    let Some(buf): Option<Vec<u8>> = fetch_udp_table(u32::from(AF_INET6.0)) else {
        return UdpLookupResult {
            available: false,
            pid: None,
        };
    };
    if buf.len() < 4 {
        return UdpLookupResult {
            available: false,
            pid: None,
        };
    }

    let count: usize = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let row_size: usize = size_of::<MIB_UDP6ROW_OWNER_PID>();
    for i in 0..count {
        let offset: usize = 4 + i * row_size;
        if offset + row_size > buf.len() {
            break;
        }
        // SAFETY: `offset + row_size <= buf.len()` guarantees `MIB_UDP6ROW_OWNER_PID` lies within `buf`, and `read_unaligned` handles arbitrary alignment.
        let row: MIB_UDP6ROW_OWNER_PID = unsafe {
            buf.as_ptr()
                .add(offset)
                .cast::<MIB_UDP6ROW_OWNER_PID>()
                .read_unaligned()
        };
        if port_from_network_dword(row.dwLocalPort) == port {
            let owner_pid: u32 = row.dwOwningPid;
            if expected_pid.is_none_or(|exp: u32| owner_pid == exp) {
                return UdpLookupResult {
                    available: true,
                    pid: Some(owner_pid),
                };
            }
        }
    }

    UdpLookupResult {
        available: true,
        pid: None,
    }
}

/// Checks both IPv4 and IPv6 Windows UDP owner-PID tables for a listener bound to `port`.
/// When `expected_pid` is `Some(pid)`, only rows owned by that PID match.
#[must_use]
pub fn find_udp_listener_pid(port: u16, expected_pid: Option<u32>) -> UdpLookupResult {
    let v4: UdpLookupResult = query_udp_v4(port, expected_pid);
    if v4.pid.is_some() {
        return v4;
    }

    let v6: UdpLookupResult = query_udp_v6(port, expected_pid);
    if v6.pid.is_some() {
        return v6;
    }

    UdpLookupResult {
        available: v4.available || v6.available,
        pid: None,
    }
}

fn fetch_tcp_listener_table(address_family: u32) -> Option<Vec<u8>> {
    let mut needed: u32 = 0;
    // SAFETY: Passing `None` buffer queries the required byte size into `&raw mut needed`.
    let status: u32 = unsafe {
        GetExtendedTcpTable(
            None,
            &raw mut needed,
            false,
            address_family,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    if status != ERROR_INSUFFICIENT_BUFFER.0 && status != NO_ERROR.0 {
        return None;
    }

    let mut alloc_size: usize =
        (needed as usize).max(IP_TABLE_MIN_BUFFER_SIZE) + IP_TABLE_EXTRA_BUFFER_SIZE;
    let mut buf: Vec<u8> = vec![0u8; alloc_size];

    for _ in 0u8..4u8 {
        let mut size_arg: u32 = u32::try_from(alloc_size).ok()?;
        // SAFETY: `buf` has length `alloc_size == size_arg as usize`, writable for `GetExtendedTcpTable`.
        let status: u32 = unsafe {
            GetExtendedTcpTable(
                Some(buf.as_mut_ptr().cast::<c_void>()),
                &raw mut size_arg,
                false,
                address_family,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if status == NO_ERROR.0 {
            return Some(buf);
        }
        if status != ERROR_INSUFFICIENT_BUFFER.0 {
            return None;
        }
        alloc_size = (size_arg as usize).max(alloc_size) + IP_TABLE_EXTRA_BUFFER_SIZE;
        buf.resize(alloc_size, 0);
    }

    None
}

fn is_tcp_v4_listening(port: u16) -> bool {
    let Some(buf): Option<Vec<u8>> = fetch_tcp_listener_table(u32::from(AF_INET.0)) else {
        return false;
    };
    if buf.len() < 4 {
        return false;
    }
    let count: usize = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let row_size: usize = size_of::<MIB_TCPROW_OWNER_PID>();
    for i in 0..count {
        let offset: usize = 4 + i * row_size;
        if offset + row_size > buf.len() {
            break;
        }
        // SAFETY: `offset + row_size <= buf.len()` guarantees `MIB_TCPROW_OWNER_PID` lies within `buf`, and `read_unaligned` handles arbitrary alignment.
        let row: MIB_TCPROW_OWNER_PID = unsafe {
            buf.as_ptr()
                .add(offset)
                .cast::<MIB_TCPROW_OWNER_PID>()
                .read_unaligned()
        };
        if port_from_network_dword(row.dwLocalPort) == port {
            return true;
        }
    }
    false
}

fn is_tcp_v6_listening(port: u16) -> bool {
    let Some(buf): Option<Vec<u8>> = fetch_tcp_listener_table(u32::from(AF_INET6.0)) else {
        return false;
    };
    if buf.len() < 4 {
        return false;
    }
    let count: usize = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let row_size: usize = size_of::<MIB_TCP6ROW_OWNER_PID>();
    for i in 0..count {
        let offset: usize = 4 + i * row_size;
        if offset + row_size > buf.len() {
            break;
        }
        // SAFETY: `offset + row_size <= buf.len()` guarantees `MIB_TCP6ROW_OWNER_PID` lies within `buf`, and `read_unaligned` handles arbitrary alignment.
        let row: MIB_TCP6ROW_OWNER_PID = unsafe {
            buf.as_ptr()
                .add(offset)
                .cast::<MIB_TCP6ROW_OWNER_PID>()
                .read_unaligned()
        };
        if port_from_network_dword(row.dwLocalPort) == port {
            return true;
        }
    }
    false
}

/// Returns `true` if any IPv4 or IPv6 TCP socket is currently listening on `port`.
#[must_use]
pub fn is_tcp_port_listening(port: u16) -> bool {
    is_tcp_v4_listening(port) || is_tcp_v6_listening(port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, UdpSocket};

    #[test]
    fn udp_listener_lookup_matches_ipv4_and_ipv6() {
        let my_pid = std::process::id();

        let sock_v4 = UdpSocket::bind("127.0.0.1:0").expect("bind ipv4 udp");
        let port_v4 = sock_v4.local_addr().unwrap().port();
        let res_v4 = find_udp_listener_pid(port_v4, Some(my_pid));
        assert!(res_v4.available);
        assert_eq!(res_v4.pid, Some(my_pid));

        // Wrong expected PID must return None
        let wrong = find_udp_listener_pid(port_v4, Some(my_pid.wrapping_add(999_999)));
        assert!(wrong.available);
        assert_eq!(wrong.pid, None);

        drop(sock_v4);

        if let Ok(sock_v6) = UdpSocket::bind("[::1]:0") {
            let port_v6 = sock_v6.local_addr().unwrap().port();
            let res_v6 = find_udp_listener_pid(port_v6, Some(my_pid));
            assert!(res_v6.available);
            assert_eq!(res_v6.pid, Some(my_pid));
            drop(sock_v6);
        }
    }

    #[test]
    fn tcp_listener_lookup_detects_bound_port() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ipv4 tcp");
        let port = listener.local_addr().unwrap().port();
        assert!(is_tcp_port_listening(port));
        drop(listener);
        assert!(!is_tcp_port_listening(port));
    }
}
