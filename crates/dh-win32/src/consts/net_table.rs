use std::mem::align_of;

use windows::Win32::NetworkManagement::IpHelper::{
    MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID, MIB_UDP6ROW_OWNER_PID, MIB_UDPROW_OWNER_PID,
};

pub const IP_TABLE_HEADER_SIZE: usize = 4;
pub const IP_TABLE_MIN_BUFFER_SIZE: usize = 4096;
pub const IP_TABLE_EXTRA_BUFFER_SIZE: usize = 2048;
pub const MAX_IP_TABLE_BUFFER_SIZE: usize = 64 * 1024 * 1024;
pub const IP_TABLE_QUERY_RETRIES: u8 = 4;

const _: () = assert!(align_of::<MIB_UDPROW_OWNER_PID>() <= IP_TABLE_HEADER_SIZE);
const _: () = assert!(align_of::<MIB_UDP6ROW_OWNER_PID>() <= IP_TABLE_HEADER_SIZE);
const _: () = assert!(align_of::<MIB_TCPROW_OWNER_PID>() <= IP_TABLE_HEADER_SIZE);
const _: () = assert!(align_of::<MIB_TCP6ROW_OWNER_PID>() <= IP_TABLE_HEADER_SIZE);
