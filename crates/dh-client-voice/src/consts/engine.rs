/// Maximum number of `DHV2` packets drained inside a single `FSocketBSD::RecvFrom` call
/// before returning control to UE4 (`WSAEWOULDBLOCK` emulation on flood).
pub const MAX_DHV2_DRAIN_PER_RECV: usize = 128;

/// UE4 `ESocketReceiveFlags::Peek` bitmask (`MSG_PEEK` = `2`).
pub const SOCKET_RECV_FLAG_PEEK: u32 = 2;

/// UE4 `ESocketType::SOCKTYPE_Datagram` value (`1` = UDP, `2` = TCP; verified in `FSocketSubsystemWindows::CreateSocket`).
pub const FSOCKET_TYPE_DATAGRAM: u32 = 1;

/// Sentinel invalid Win32 `SOCKET` handle (`INVALID_SOCKET = !0usize`).
pub const INVALID_RAW_SOCKET: usize = usize::MAX;

/// Size in bytes of Win32 `SOCKADDR_STORAGE` (`128` bytes).
pub const SOCKADDR_STORAGE_SIZE: usize = 128;

/// `WinSock` `AF_INET` address family (`2`).
pub const SOCKADDR_AF_INET: u16 = 2;

/// `WinSock` `AF_INET6` address family (`23`).
pub const SOCKADDR_AF_INET6: u16 = 23;

/// Byte length of `WinSock` `SOCKADDR_IN` (`16` bytes).
pub const SOCKADDR_INET_LEN: i32 = 16;

/// Byte length of `WinSock` `SOCKADDR_IN6` (`28` bytes).
pub const SOCKADDR_INET6_LEN: i32 = 28;

/// Fast retry cadence in milliseconds for `DHV2::Hello` while awaiting initial `HelloAck`.
pub const HELLO_RETRY_INTERVAL_MS: u64 = 1_000;

/// Steady-state keepalive cadence in milliseconds for `DHV2::Hello` once `peer_slot` is assigned.
pub const HELLO_KEEPALIVE_INTERVAL_MS: u64 = 3_000;

/// Maximum UTF-16 code units permitted when converting UE4 `FString` unique IDs.
pub const MAX_UE4_UID_UTF16_LEN: usize = 256;
