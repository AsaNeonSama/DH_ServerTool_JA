//! Verified RVAs, struct offsets, vtable RVAs, and x86-64 trampoline encoding constants for
//! `DreadHungerServer-Win64-Shipping.exe`.

/// Target dedicated server executable filename (case-insensitive guard).
pub const TARGET_PROCESS_NAME: &str = "DreadHungerServer-Win64-Shipping.exe";

/// Optional environment variable enabling relative diagnostic logging (`./dh_voice_server.log`).
pub const ENV_ENABLE_LOG: &str = "DH_VOICE_SERVER_LOG";

/// Relative diagnostic log file path (created only when `DH_VOICE_SERVER_LOG=1`).
pub const RELATIVE_LOG_FILE: &str = "dh_voice_server.log";

/// RVA of `FSocketBSD::RecvFrom` in `DreadHungerServer-Win64-Shipping.exe`.
pub const RVA_FSOCKET_BSD_RECV_FROM: usize = 0x0158_A1F0;

/// Verified safe 5-byte prologue of `FSocketBSD::RecvFrom` (`mov [rsp+8], rbx`, zero RIP-rel).
pub const EXPECTED_RECV_FROM_PROLOGUE: [u8; 5] = [0x48, 0x89, 0x5C, 0x24, 0x08];

/// RVA of `FSocketBSD::Close` in `DreadHungerServer-Win64-Shipping.exe`.
pub const RVA_FSOCKET_BSD_CLOSE: usize = 0x0158_49C0;

/// Verified safe 6-byte prologue of `FSocketBSD::Close` (`push rbx; sub rsp, 20h`, zero RIP-rel).
pub const EXPECTED_CLOSE_PROLOGUE: [u8; 6] = [0x40, 0x53, 0x48, 0x83, 0xEC, 0x20];

/// RVA of `UNetConnection::ReceivedPacket` in `DreadHungerServer-Win64-Shipping.exe`.
///
/// Only reached AFTER UE4's `StatelessConnectHandlerComponent` 3-way UDP handshake succeeds,
/// a live `UIpConnection` exists, and `PacketHandler::Incoming_Internal` validates the packet.
pub const RVA_UNET_CONNECTION_RECEIVED_PACKET: usize = 0x0268_2090;

/// Verified safe 6-byte prologue of `UNetConnection::ReceivedPacket`
/// (`push rbp; push r12; push r13`, zero RIP-rel).
pub const EXPECTED_RECEIVED_PACKET_PROLOGUE: [u8; 6] = [0x40, 0x55, 0x41, 0x54, 0x41, 0x55];

/// Primary vtable RVA of `UIpConnection`.
pub const RVA_UIP_CONNECTION_VTABLE: usize = 0x032C_5A80;

/// Primary vtable RVA of `UNetConnectionEOS` (subclass of `UIpConnection`).
pub const RVA_UNET_CONNECTION_EOS_VTABLE: usize = 0x0330_0E18;

/// Primary vtable RVA of `USteamNetConnection` (subclass of `UIpConnection`).
pub const RVA_USTEAM_NET_CONNECTION_VTABLE: usize = 0x032E_DCE0;

/// Primary vtable RVA of `FInternetAddrBSD`.
pub const RVA_FINTERNET_ADDR_BSD_VTABLE: usize = 0x0363_C310;

/// Primary vtable RVA of `FSocketBSD`.
pub const RVA_FSOCKET_BSD_VTABLE: usize = 0x0363_C680;

/// Byte offset of `UNetConnection::RemoteAddr` (`TSharedPtr<FInternetAddr>::Object`).
pub const OFFSET_UNET_CONNECTION_REMOTE_ADDR: usize = 0x0110;

/// Byte offset of `UIpConnection::SocketPrivate` (`FSocket*`).
pub const OFFSET_UIP_CONNECTION_SOCKET: usize = 0x1B58;

/// Byte offset of `FSocketBSD::SocketType` (`ESocketType: u32`, `1 = SOCKTYPE_Datagram`, `2 = SOCKTYPE_Streaming`).
pub const OFFSET_FSOCKET_SOCKET_TYPE: usize = 0x08;

/// `ESocketType::SOCKTYPE_Datagram` (UDP = `1`; TCP `SOCKTYPE_Streaming` = `2`).
pub const ESOCKET_TYPE_DATAGRAM: u32 = 1;

/// Byte offset of Win32 `SOCKET` (`usize`) inside `FSocketBSD`.
pub const OFFSET_FSOCKET_RAW_SOCKET: usize = 0x28;

/// Byte offset of `SOCKADDR_STORAGE` (128 bytes) inside `FInternetAddrBSD`.
pub const OFFSET_FINTERNET_ADDR_STORAGE: usize = 0x08;

/// `ESocketReceiveFlags::Peek` bitmask in `FSocketBSD::RecvFrom`.
pub const ESOCKET_RECEIVE_FLAG_PEEK: u32 = 2;

/// `AF_INET` address family constant (`2`).
pub const ADDR_FAMILY_INET: u16 = 2;

/// `AF_INET6` address family constant (`23`).
pub const ADDR_FAMILY_INET6: u16 = 23;

/// Size of `WinSock` `SOCKADDR_IN` in bytes (`16`).
pub const SOCKADDR_IN_LEN: i32 = 16;

/// Size of `WinSock` `SOCKADDR_IN6` in bytes (`28`).
pub const SOCKADDR_IN6_LEN: i32 = 28;

/// Size of `WinSock` `SOCKADDR_STORAGE` in bytes (`128`).
pub const SOCKADDR_STORAGE_LEN: usize = 128;

/// PE DOS header magic (`"MZ"`).
pub const PE_DOS_MAGIC: u16 = 0x5A4D;

/// Offset of `e_lfanew` inside `IMAGE_DOS_HEADER`.
pub const PE_LFANEW_OFFSET: usize = 0x3C;

/// Minimum DOS header bytes required to read `e_lfanew`.
pub const PE_MIN_DOS_BYTES: usize = 0x40;

/// PE NT signature (`"PE\0\0"`).
pub const PE_NT_SIGNATURE: u32 = 0x0000_4550;

/// Offset of `IMAGE_OPTIONAL_HEADER64` from the start of `IMAGE_NT_HEADERS64`.
pub const PE_OPTIONAL_HEADER_OFFSET: usize = 0x18;

/// PE32+ optional header magic.
pub const PE_OPT_MAGIC_PE32_PLUS: u16 = 0x020B;

/// Offset of `SizeOfImage` from the start of `IMAGE_OPTIONAL_HEADER64`.
pub const PE_SIZE_OF_IMAGE_OFFSET: usize = 0x38;

/// Minimum bytes from `e_lfanew` required to read `SizeOfImage`.
pub const PE_MIN_NT_BYTES: usize = PE_OPTIONAL_HEADER_OFFSET + PE_SIZE_OF_IMAGE_OFFSET + 4;

/// Size of our single nearby `RX` trampoline page (`4 KiB`).
pub const TRAMPOLINE_ALLOC_SIZE: usize = 0x1000;

/// Default Windows virtual allocation granularity (`64 KiB`).
pub const DEFAULT_ALLOC_GRANULARITY: usize = 0x10000;

/// Maximum distance from module base for 32-bit relative `jmp rel32` reachability (`1.75 GiB`).
pub const TRAMPOLINE_MAX_DISTANCE: usize = 0x7000_0000;

/// x86-64 near jump opcode (`jmp rel32`).
pub const X64_JMP_REL32_OPCODE: u8 = 0xE9;

/// Length of `jmp rel32` in bytes.
pub const X64_JMP_REL32_LEN: usize = 5;

/// Prefix bytes for 14-byte absolute indirect jump `jmp qword ptr [rip + 0]` (`FF 25 00 00 00 00`).
pub const X64_JMP_ABS_INDIRECT_PREFIX: [u8; 6] = [0xFF, 0x25, 0x00, 0x00, 0x00, 0x00];

/// Total length of `jmp qword ptr [rip + 0]; dq target64` (`14` bytes).
pub const X64_JMP_ABS_INDIRECT_LEN: usize = 14;

/// x86-64 `nop` byte.
pub const X64_NOP_BYTE: u8 = 0x90;

/// x86-64 `int3` padding byte for unused trampoline page space.
pub const X64_INT3_BYTE: u8 = 0xCC;

/// Offset of `RecvFrom` entry bridge inside the `RX` trampoline page.
pub const TRAMP_OFF_RECV_BRIDGE: usize = 0x00;

/// Offset of `RecvFrom` original-function trampoline inside the `RX` trampoline page.
pub const TRAMP_OFF_RECV_ORIG: usize = 0x20;

/// Offset of `Close` entry bridge inside the `RX` trampoline page.
pub const TRAMP_OFF_CLOSE_BRIDGE: usize = 0x40;

/// Offset of `Close` original-function trampoline inside the `RX` trampoline page.
pub const TRAMP_OFF_CLOSE_ORIG: usize = 0x60;

/// Offset of `UNetConnection::ReceivedPacket` entry bridge inside the `RX` trampoline page.
pub const TRAMP_OFF_RECV_PKT_BRIDGE: usize = 0x80;

/// Offset of `UNetConnection::ReceivedPacket` original-function trampoline inside the `RX` page.
pub const TRAMP_OFF_RECV_PKT_ORIG: usize = 0xA0;

/// Initial UTF-16 buffer capacity for `GetModuleFileNameW`.
pub const WIN32_PATH_BUF_CAPACITY: usize = 512;

/// Maximum UTF-16 buffer capacity for `GetModuleFileNameW`.
pub const WIN32_PATH_BUF_MAX: usize = 32_768;
