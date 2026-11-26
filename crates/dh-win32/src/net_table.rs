use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
    MIB_UDP6ROW_OWNER_PID, MIB_UDPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

use crate::consts::net_table::{
    IP_TABLE_EXTRA_BUFFER_SIZE, IP_TABLE_HEADER_SIZE, IP_TABLE_MIN_BUFFER_SIZE,
    IP_TABLE_QUERY_RETRIES, MAX_IP_TABLE_BUFFER_SIZE,
};
pub use crate::structs::net_table::UdpLookupResult;

#[inline]
fn port_from_network_dword(raw_port: u32) -> u16 {
    let low_word: u16 = u16::try_from(raw_port & 0xFFFF).unwrap_or(0);
    u16::from_be(low_word)
}

fn fetch_ip_table<F>(mut query: F) -> Option<Vec<u8>>
where
    F: FnMut(Option<*mut c_void>, &mut u32) -> u32,
{
    let mut needed: u32 = 0;
    let initial_status: u32 = query(None, &mut needed);
    if initial_status != ERROR_INSUFFICIENT_BUFFER.0 && initial_status != NO_ERROR.0 {
        return None;
    }

    let mut alloc_size: usize = (needed as usize)
        .max(IP_TABLE_MIN_BUFFER_SIZE)
        .checked_add(IP_TABLE_EXTRA_BUFFER_SIZE)?;
    if alloc_size > MAX_IP_TABLE_BUFFER_SIZE {
        return None;
    }
    let mut buf: Vec<u8> = vec![0u8; alloc_size];

    for _ in 0u8..IP_TABLE_QUERY_RETRIES {
        let mut size_arg: u32 = u32::try_from(alloc_size).ok()?;
        let status: u32 = query(Some(buf.as_mut_ptr().cast::<c_void>()), &mut size_arg);
        if status == NO_ERROR.0 {
            return Some(buf);
        }
        if status != ERROR_INSUFFICIENT_BUFFER.0 {
            return None;
        }
        alloc_size = (size_arg as usize)
            .max(alloc_size)
            .checked_add(IP_TABLE_EXTRA_BUFFER_SIZE)?;
        if alloc_size > MAX_IP_TABLE_BUFFER_SIZE {
            return None;
        }
        buf.resize(alloc_size, 0);
    }

    None
}

fn fetch_udp_table(address_family: u32) -> Option<Vec<u8>> {
    fetch_ip_table(|buf_ptr: Option<*mut c_void>, size: &mut u32| {
        // SAFETY: When `buf_ptr` is `None`, queries required byte count into `size`; when `Some(ptr)`,
        // `ptr` points to `*size as usize` writable bytes owned by `fetch_ip_table`.
        unsafe {
            GetExtendedUdpTable(
                buf_ptr,
                &raw mut *size,
                false,
                address_family,
                UDP_TABLE_OWNER_PID,
                0,
            )
        }
    })
}

fn fetch_tcp_listener_table(address_family: u32) -> Option<Vec<u8>> {
    fetch_ip_table(|buf_ptr: Option<*mut c_void>, size: &mut u32| {
        // SAFETY: When `buf_ptr` is `None`, queries required byte count into `size`; when `Some(ptr)`,
        // `ptr` points to `*size as usize` writable bytes owned by `fetch_ip_table`.
        unsafe {
            GetExtendedTcpTable(
                buf_ptr,
                &raw mut *size,
                false,
                address_family,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        }
    })
}

fn scan_ip_table_rows<R: Copy, T, F>(buf: &[u8], mut inspect: F) -> Option<T>
where
    F: FnMut(R) -> Option<T>,
{
    let row_size: usize = size_of::<R>();
    if row_size == 0 || buf.len() < IP_TABLE_HEADER_SIZE {
        return None;
    }

    let declared_count: usize = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let max_rows: usize = buf.len().saturating_sub(IP_TABLE_HEADER_SIZE) / row_size;
    let safe_count: usize = declared_count.min(max_rows);

    for idx in 0..safe_count {
        let Some(row_bytes): Option<usize> = idx.checked_mul(row_size) else {
            break;
        };
        let Some(offset): Option<usize> = IP_TABLE_HEADER_SIZE.checked_add(row_bytes) else {
            break;
        };
        let Some(end): Option<usize> = offset.checked_add(row_size) else {
            break;
        };
        if end > buf.len() {
            break;
        }
        // SAFETY: `offset + size_of::<R>() <= buf.len()` guarantees `R` (a POD Win32 MIB row struct)
        // lies strictly within `buf`, and `read_unaligned` handles arbitrary alignment.
        let row: R = unsafe { buf.as_ptr().add(offset).cast::<R>().read_unaligned() };
        if let Some(found) = inspect(row) {
            return Some(found);
        }
    }

    None
}

fn query_udp_family<R: Copy, F>(
    address_family: u32,
    port: u16,
    expected_pid: Option<u32>,
    extract: F,
) -> UdpLookupResult
where
    F: Fn(R) -> (u32, u32),
{
    let Some(buf): Option<Vec<u8>> = fetch_udp_table(address_family) else {
        return UdpLookupResult {
            available: false,
            pid: None,
        };
    };
    if buf.len() < IP_TABLE_HEADER_SIZE {
        return UdpLookupResult {
            available: false,
            pid: None,
        };
    }

    let matched_pid: Option<u32> = scan_ip_table_rows::<R, u32, _>(&buf, |row: R| {
        let (raw_port, owner_pid): (u32, u32) = extract(row);
        if port_from_network_dword(raw_port) == port
            && expected_pid.is_none_or(|exp: u32| owner_pid == exp)
        {
            Some(owner_pid)
        } else {
            None
        }
    });

    UdpLookupResult {
        available: true,
        pid: matched_pid,
    }
}

/// Checks both IPv4 and IPv6 Windows UDP owner-PID tables for a listener bound to `port`.
/// When `expected_pid` is `Some(pid)`, only rows owned by that PID match.
#[must_use]
pub fn find_udp_listener_pid(port: u16, expected_pid: Option<u32>) -> UdpLookupResult {
    let v4: UdpLookupResult = query_udp_family::<MIB_UDPROW_OWNER_PID, _>(
        u32::from(AF_INET.0),
        port,
        expected_pid,
        |row: MIB_UDPROW_OWNER_PID| (row.dwLocalPort, row.dwOwningPid),
    );
    if v4.pid.is_some() {
        return v4;
    }

    let v6: UdpLookupResult = query_udp_family::<MIB_UDP6ROW_OWNER_PID, _>(
        u32::from(AF_INET6.0),
        port,
        expected_pid,
        |row: MIB_UDP6ROW_OWNER_PID| (row.dwLocalPort, row.dwOwningPid),
    );
    if v6.pid.is_some() {
        return v6;
    }

    UdpLookupResult {
        available: v4.available || v6.available,
        pid: None,
    }
}

fn is_tcp_family_listening<R: Copy, F>(address_family: u32, port: u16, local_port: F) -> bool
where
    F: Fn(R) -> u32,
{
    let Some(buf): Option<Vec<u8>> = fetch_tcp_listener_table(address_family) else {
        return false;
    };
    scan_ip_table_rows::<R, (), _>(&buf, |row: R| {
        (port_from_network_dword(local_port(row)) == port).then_some(())
    })
    .is_some()
}

/// Returns `true` if any IPv4 or IPv6 TCP socket is currently listening on `port`.
#[must_use]
pub fn is_tcp_port_listening(port: u16) -> bool {
    is_tcp_family_listening::<MIB_TCPROW_OWNER_PID, _>(
        u32::from(AF_INET.0),
        port,
        |row: MIB_TCPROW_OWNER_PID| row.dwLocalPort,
    ) || is_tcp_family_listening::<MIB_TCP6ROW_OWNER_PID, _>(
        u32::from(AF_INET6.0),
        port,
        |row: MIB_TCP6ROW_OWNER_PID| row.dwLocalPort,
    )
}
