//! Live `FSocketBSD::RecvFrom`, `FSocketBSD::Close`, and post-handshake
//! `UNetConnection::ReceivedPacket` hooks for `DreadHungerServer-Win64-Shipping.exe`.

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(windows)]
use std::sync::{Mutex, OnceLock};

#[cfg(windows)]
use dh_voice_proto::is_dhv2_packet;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::HMODULE;
#[cfg(windows)]
use windows::Win32::Networking::WinSock::{sendto, WSAGetLastError, SOCKADDR, SOCKET};
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
#[cfg(windows)]
use windows::Win32::System::SystemInformation::GetTickCount64;

#[cfg(windows)]
use crate::consts::patch::{
    ESOCKET_RECEIVE_FLAG_PEEK, ESOCKET_TYPE_DATAGRAM, EXPECTED_CLOSE_PROLOGUE,
    EXPECTED_RECEIVED_PACKET_PROLOGUE, EXPECTED_RECV_FROM_PROLOGUE, OFFSET_FINTERNET_ADDR_STORAGE,
    OFFSET_FSOCKET_RAW_SOCKET, OFFSET_FSOCKET_SOCKET_TYPE, OFFSET_UIP_CONNECTION_SOCKET,
    OFFSET_UNET_CONNECTION_REMOTE_ADDR, RVA_FINTERNET_ADDR_BSD_VTABLE, RVA_FSOCKET_BSD_CLOSE,
    RVA_FSOCKET_BSD_RECV_FROM, RVA_FSOCKET_BSD_VTABLE, RVA_UIP_CONNECTION_VTABLE,
    RVA_UNET_CONNECTION_EOS_VTABLE, RVA_UNET_CONNECTION_RECEIVED_PACKET,
    RVA_USTEAM_NET_CONNECTION_VTABLE, SOCKADDR_STORAGE_LEN, TRAMP_OFF_CLOSE_BRIDGE,
    TRAMP_OFF_CLOSE_ORIG, TRAMP_OFF_RECV_BRIDGE, TRAMP_OFF_RECV_ORIG, TRAMP_OFF_RECV_PKT_BRIDGE,
    TRAMP_OFF_RECV_PKT_ORIG, WIN32_PATH_BUF_CAPACITY, WIN32_PATH_BUF_MAX, X64_INT3_BYTE,
};
#[cfg(windows)]
use crate::consts::relay::MAX_DHV2_DRAIN_PER_CALL;
#[cfg(windows)]
use crate::logger::{is_logging_enabled, log_line};
#[cfg(windows)]
use crate::patcher::{
    allocate_nearby_rw_page, encode_detour_site_patch, encode_jmp_abs_indirect,
    encode_original_trampoline, is_target_process_path, read_live_image_size,
    seal_trampoline_page_rx, write_protected_code,
};
#[cfg(windows)]
use crate::structs::patch::DetourError;
use crate::structs::patch::HookInstallReport;
#[cfg(windows)]
use crate::structs::relay::{
    AlignedSockAddr, OutboundControlPacket, PeerEndpoint, RelayDispatch, RelayEngine,
};

#[cfg(windows)]
type RecvFromFn =
    unsafe extern "system" fn(*mut c_void, *mut u8, i32, *mut i32, *mut c_void, u32) -> u8;

#[cfg(windows)]
type CloseFn = unsafe extern "system" fn(*mut c_void) -> u8;

#[cfg(windows)]
type ReceivedPacketFn = unsafe extern "system" fn(*mut c_void, *mut c_void, u8);

#[cfg(windows)]
static HOOKS_INITIALIZED: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static FIRST_RECV_LOGGED: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static FIRST_UNAUTH_DROP_LOGGED: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static MODULE_BASE: AtomicUsize = AtomicUsize::new(0);
#[cfg(windows)]
static ORIG_RECV_FROM: AtomicUsize = AtomicUsize::new(0);
#[cfg(windows)]
static ORIG_CLOSE: AtomicUsize = AtomicUsize::new(0);
#[cfg(windows)]
static ORIG_RECEIVED_PACKET: AtomicUsize = AtomicUsize::new(0);
#[cfg(windows)]
static LAST_REPORT: OnceLock<HookInstallReport> = OnceLock::new();
#[cfg(windows)]
static RELAY_ENGINE: OnceLock<Mutex<RelayEngine>> = OnceLock::new();

/// Returns the cached hook installation report from initialization, if available.
#[must_use]
pub fn last_install_report() -> Option<HookInstallReport> {
    #[cfg(windows)]
    {
        LAST_REPORT.get().copied()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Installs the `FSocketBSD::RecvFrom`, `FSocketBSD::Close`, and post-handshake
/// `UNetConnection::ReceivedPacket` detours if the current process is
/// `DreadHungerServer-Win64-Shipping.exe`.
///
/// Safe to invoke from `DllMain` (`DLL_PROCESS_ATTACH`): performs no `LoadLibrary`, thread spawn,
/// or `WinSock` calls during installation, and leaves zero persistent `RWX` pages.
#[cfg(windows)]
#[must_use]
pub fn install_server_hooks_if_target() -> HookInstallReport {
    if HOOKS_INITIALIZED.swap(true, Ordering::AcqRel) {
        return LAST_REPORT.get().copied().unwrap_or(HookInstallReport {
            target_matched: false,
            hooks_installed: 0,
            error: None,
        });
    }

    let report: HookInstallReport = install_server_hooks_inner();
    let _stored: Result<(), HookInstallReport> = LAST_REPORT.set(report);
    report
}

/// Non-Windows stub returning an inert report.
#[cfg(not(windows))]
#[must_use]
pub fn install_server_hooks_if_target() -> HookInstallReport {
    HookInstallReport {
        target_matched: false,
        hooks_installed: 0,
        error: None,
    }
}

#[cfg(windows)]
fn install_server_hooks_inner() -> HookInstallReport {
    let Some(exe_path): Option<String> = current_process_exe_path() else {
        return HookInstallReport {
            target_matched: false,
            hooks_installed: 0,
            error: None,
        };
    };

    if !is_target_process_path(&exe_path) {
        return HookInstallReport {
            target_matched: false,
            hooks_installed: 0,
            error: None,
        };
    }

    // SAFETY: Passing `PCWSTR::null()` returns the base handle of the main executable image
    // already loaded in the current process.
    let Ok(module_handle): Result<HMODULE, windows::core::Error> =
        (unsafe { GetModuleHandleW(PCWSTR::null()) })
    else {
        return HookInstallReport {
            target_matched: true,
            hooks_installed: 0,
            error: Some(DetourError::MemoryError),
        };
    };

    let module_base: usize = module_handle.0 as usize;
    let Some(image_size): Option<usize> = read_live_image_size(module_base) else {
        return HookInstallReport {
            target_matched: true,
            hooks_installed: 0,
            error: Some(DetourError::OutOfBounds),
        };
    };

    match install_detours(module_base, image_size) {
        Ok(()) => {
            log_line(
                "installed FSocketBSD::RecvFrom, FSocketBSD::Close, and UNetConnection::ReceivedPacket detours",
            );
            HookInstallReport {
                target_matched: true,
                hooks_installed: 3,
                error: None,
            }
        }
        Err(err) => {
            log_line(&format!("failed to install detours: {err:?}"));
            HookInstallReport {
                target_matched: true,
                hooks_installed: 0,
                error: Some(err),
            }
        }
    }
}

#[cfg(windows)]
fn install_detours(module_base: usize, image_size: usize) -> Result<(), DetourError> {
    let recv_end: usize = RVA_FSOCKET_BSD_RECV_FROM
        .checked_add(EXPECTED_RECV_FROM_PROLOGUE.len())
        .ok_or(DetourError::OutOfBounds)?;
    let close_end: usize = RVA_FSOCKET_BSD_CLOSE
        .checked_add(EXPECTED_CLOSE_PROLOGUE.len())
        .ok_or(DetourError::OutOfBounds)?;
    let recv_pkt_end: usize = RVA_UNET_CONNECTION_RECEIVED_PACKET
        .checked_add(EXPECTED_RECEIVED_PACKET_PROLOGUE.len())
        .ok_or(DetourError::OutOfBounds)?;
    if recv_end > image_size || close_end > image_size || recv_pkt_end > image_size {
        return Err(DetourError::OutOfBounds);
    }

    let recv_addr: usize = module_base
        .checked_add(RVA_FSOCKET_BSD_RECV_FROM)
        .ok_or(DetourError::AddressOverflow)?;
    let close_addr: usize = module_base
        .checked_add(RVA_FSOCKET_BSD_CLOSE)
        .ok_or(DetourError::AddressOverflow)?;
    let recv_pkt_addr: usize = module_base
        .checked_add(RVA_UNET_CONNECTION_RECEIVED_PACKET)
        .ok_or(DetourError::AddressOverflow)?;

    // SAFETY: All three target addresses are bounds-checked within `[module_base, module_base + image_size)`.
    let recv_prologue_live: &[u8] = unsafe {
        std::slice::from_raw_parts(recv_addr as *const u8, EXPECTED_RECV_FROM_PROLOGUE.len())
    };
    if recv_prologue_live != EXPECTED_RECV_FROM_PROLOGUE {
        return Err(DetourError::SignatureMismatch);
    }

    // SAFETY: `close_addr` is bounds-checked within `[module_base, module_base + image_size)`.
    let close_prologue_live: &[u8] = unsafe {
        std::slice::from_raw_parts(close_addr as *const u8, EXPECTED_CLOSE_PROLOGUE.len())
    };
    if close_prologue_live != EXPECTED_CLOSE_PROLOGUE {
        return Err(DetourError::SignatureMismatch);
    }

    // SAFETY: `recv_pkt_addr` is bounds-checked within `[module_base, module_base + image_size)`.
    let recv_pkt_prologue_live: &[u8] = unsafe {
        std::slice::from_raw_parts(
            recv_pkt_addr as *const u8,
            EXPECTED_RECEIVED_PACKET_PROLOGUE.len(),
        )
    };
    if recv_pkt_prologue_live != EXPECTED_RECEIVED_PACKET_PROLOGUE {
        return Err(DetourError::SignatureMismatch);
    }

    let tramp_page: usize =
        allocate_nearby_rw_page(module_base, image_size).ok_or(DetourError::MemoryError)?;

    let recv_bridge_addr: usize = tramp_page
        .checked_add(TRAMP_OFF_RECV_BRIDGE)
        .ok_or(DetourError::AddressOverflow)?;
    let recv_orig_addr: usize = tramp_page
        .checked_add(TRAMP_OFF_RECV_ORIG)
        .ok_or(DetourError::AddressOverflow)?;
    let close_bridge_addr: usize = tramp_page
        .checked_add(TRAMP_OFF_CLOSE_BRIDGE)
        .ok_or(DetourError::AddressOverflow)?;
    let close_orig_addr: usize = tramp_page
        .checked_add(TRAMP_OFF_CLOSE_ORIG)
        .ok_or(DetourError::AddressOverflow)?;
    let recv_pkt_bridge_addr: usize = tramp_page
        .checked_add(TRAMP_OFF_RECV_PKT_BRIDGE)
        .ok_or(DetourError::AddressOverflow)?;
    let recv_pkt_orig_addr: usize = tramp_page
        .checked_add(TRAMP_OFF_RECV_PKT_ORIG)
        .ok_or(DetourError::AddressOverflow)?;

    let recv_hook_addr: usize = hooked_recv_from as *const () as usize;
    let close_hook_addr: usize = hooked_close as *const () as usize;
    let recv_pkt_hook_addr: usize = hooked_received_packet as *const () as usize;

    let recv_bridge_bytes: [u8; 14] = encode_jmp_abs_indirect(recv_hook_addr)?;
    let recv_resume_addr: usize = recv_addr
        .checked_add(EXPECTED_RECV_FROM_PROLOGUE.len())
        .ok_or(DetourError::AddressOverflow)?;
    let recv_orig_bytes: Vec<u8> =
        encode_original_trampoline(&EXPECTED_RECV_FROM_PROLOGUE, recv_resume_addr)?;

    let close_bridge_bytes: [u8; 14] = encode_jmp_abs_indirect(close_hook_addr)?;
    let close_resume_addr: usize = close_addr
        .checked_add(EXPECTED_CLOSE_PROLOGUE.len())
        .ok_or(DetourError::AddressOverflow)?;
    let close_orig_bytes: Vec<u8> =
        encode_original_trampoline(&EXPECTED_CLOSE_PROLOGUE, close_resume_addr)?;

    let recv_pkt_bridge_bytes: [u8; 14] = encode_jmp_abs_indirect(recv_pkt_hook_addr)?;
    let recv_pkt_resume_addr: usize = recv_pkt_addr
        .checked_add(EXPECTED_RECEIVED_PACKET_PROLOGUE.len())
        .ok_or(DetourError::AddressOverflow)?;
    let recv_pkt_orig_bytes: Vec<u8> =
        encode_original_trampoline(&EXPECTED_RECEIVED_PACKET_PROLOGUE, recv_pkt_resume_addr)?;

    let recv_site_patch: Vec<u8> = encode_detour_site_patch(
        recv_addr,
        recv_bridge_addr,
        EXPECTED_RECV_FROM_PROLOGUE.len(),
    )?;
    let close_site_patch: Vec<u8> =
        encode_detour_site_patch(close_addr, close_bridge_addr, EXPECTED_CLOSE_PROLOGUE.len())?;
    let recv_pkt_site_patch: Vec<u8> = encode_detour_site_patch(
        recv_pkt_addr,
        recv_pkt_bridge_addr,
        EXPECTED_RECEIVED_PACKET_PROLOGUE.len(),
    )?;

    let mut page_payload: [u8; 0x100] = [X64_INT3_BYTE; 0x100];
    page_payload[TRAMP_OFF_RECV_BRIDGE..TRAMP_OFF_RECV_BRIDGE + recv_bridge_bytes.len()]
        .copy_from_slice(&recv_bridge_bytes);
    page_payload[TRAMP_OFF_RECV_ORIG..TRAMP_OFF_RECV_ORIG + recv_orig_bytes.len()]
        .copy_from_slice(&recv_orig_bytes);
    page_payload[TRAMP_OFF_CLOSE_BRIDGE..TRAMP_OFF_CLOSE_BRIDGE + close_bridge_bytes.len()]
        .copy_from_slice(&close_bridge_bytes);
    page_payload[TRAMP_OFF_CLOSE_ORIG..TRAMP_OFF_CLOSE_ORIG + close_orig_bytes.len()]
        .copy_from_slice(&close_orig_bytes);
    page_payload
        [TRAMP_OFF_RECV_PKT_BRIDGE..TRAMP_OFF_RECV_PKT_BRIDGE + recv_pkt_bridge_bytes.len()]
        .copy_from_slice(&recv_pkt_bridge_bytes);
    page_payload[TRAMP_OFF_RECV_PKT_ORIG..TRAMP_OFF_RECV_PKT_ORIG + recv_pkt_orig_bytes.len()]
        .copy_from_slice(&recv_pkt_orig_bytes);

    seal_trampoline_page_rx(tramp_page, &page_payload)?;

    // Publish module base and original trampoline addresses BEFORE patching target entries.
    MODULE_BASE.store(module_base, Ordering::Release);
    ORIG_RECV_FROM.store(recv_orig_addr, Ordering::Release);
    ORIG_CLOSE.store(close_orig_addr, Ordering::Release);
    ORIG_RECEIVED_PACKET.store(recv_pkt_orig_addr, Ordering::Release);

    write_protected_code(recv_addr, &recv_site_patch)?;
    write_protected_code(close_addr, &close_site_patch)?;
    write_protected_code(recv_pkt_addr, &recv_pkt_site_patch)?;
    Ok(())
}

#[cfg(windows)]
fn relay_engine() -> &'static Mutex<RelayEngine> {
    RELAY_ENGINE.get_or_init(|| Mutex::new(RelayEngine::new()))
}

#[cfg(windows)]
fn current_process_exe_path() -> Option<String> {
    let mut capacity: usize = WIN32_PATH_BUF_CAPACITY;
    while capacity <= WIN32_PATH_BUF_MAX {
        let mut buf: Vec<u16> = vec![0u16; capacity];
        // SAFETY: `buf` is a writable UTF-16 slice of length `capacity`, and `None` queries the
        // current process image path.
        let written: u32 = unsafe { GetModuleFileNameW(None, &mut buf) };
        if written == 0 {
            return None;
        }
        let written_usize: usize = usize::try_from(written).ok()?;
        if written_usize < capacity {
            return Some(String::from_utf16_lossy(&buf[..written_usize]));
        }
        capacity = capacity.checked_mul(2)?;
    }
    None
}

#[cfg(windows)]
fn send_udp_packet(raw_socket: usize, target: PeerEndpoint, payload: &[u8]) {
    let (raw_addr, addr_len): ([u8; 28], i32) = target.to_sockaddr_bytes();
    let aligned: AlignedSockAddr = AlignedSockAddr { bytes: raw_addr };
    let sockaddr_ptr: *const SOCKADDR = (&raw const aligned).cast::<SOCKADDR>();
    // SAFETY: `raw_socket` is the live Win32 UDP socket handle owned by `FSocketBSD`, `payload`
    // is a valid byte slice, and `aligned` holds an 8-byte-aligned `SOCKADDR_IN`/`SOCKADDR_IN6`
    // of exact length `addr_len`.
    let sent: i32 = unsafe { sendto(SOCKET(raw_socket), payload, 0, sockaddr_ptr, addr_len) };
    if sent < 0 && is_logging_enabled() {
        // SAFETY: `WSAGetLastError` reads thread-local WinSock error code after failed `sendto`.
        let wsa_err: i32 = unsafe { WSAGetLastError().0 };
        log_line(&format!(
            "sendto failed: raw_socket={raw_socket} len={} wsa_err={wsa_err}",
            payload.len()
        ));
    }
}

#[cfg(windows)]
unsafe fn read_usize_at(base_ptr: *const c_void, offset: usize) -> usize {
    // SAFETY: Caller guarantees `base_ptr + offset` points to a readable `usize`-sized field.
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            base_ptr.cast::<u8>().add(offset),
            std::mem::size_of::<usize>(),
        )
    };
    let mut raw: [u8; std::mem::size_of::<usize>()] = [0u8; std::mem::size_of::<usize>()];
    raw.copy_from_slice(bytes);
    usize::from_ne_bytes(raw)
}

#[cfg(windows)]
unsafe fn read_socket_type(socket_ptr: *const c_void) -> u32 {
    // SAFETY: Caller verified `socket_ptr` is a valid non-null `FSocketBSD*`, and `SocketType`
    // (`ESocketType: u32`) resides at verified offset `+0x08`.
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            socket_ptr.cast::<u8>().add(OFFSET_FSOCKET_SOCKET_TYPE),
            std::mem::size_of::<u32>(),
        )
    };
    u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(windows)]
unsafe fn read_raw_socket(socket_ptr: *const c_void) -> usize {
    // SAFETY: Caller verified `socket_ptr` is a valid non-null `FSocketBSD*`, and Win32 `SOCKET`
    // resides at verified offset `+0x28`.
    unsafe { read_usize_at(socket_ptr, OFFSET_FSOCKET_RAW_SOCKET) }
}

#[cfg(windows)]
unsafe fn extract_peer_endpoint(source_addr_ptr: *const c_void) -> Option<PeerEndpoint> {
    if source_addr_ptr.is_null() {
        return None;
    }
    // SAFETY: `FInternetAddrBSD::Addr` (`SOCKADDR_STORAGE`, 128 bytes) resides at verified offset
    // `+0x08` inside `FInternetAddrBSD`.
    let storage_slice: &[u8] = unsafe {
        let storage_ptr: *const u8 = source_addr_ptr
            .cast::<u8>()
            .add(OFFSET_FINTERNET_ADDR_STORAGE);
        std::slice::from_raw_parts(storage_ptr, SOCKADDR_STORAGE_LEN)
    };
    PeerEndpoint::from_sockaddr_bytes(storage_slice)
}

/// Post-handshake authorization hook on `UNetConnection::ReceivedPacket`.
///
/// Only invoked AFTER UE4's `StatelessConnectHandlerComponent` 3-way UDP handshake succeeds and
/// `PacketHandler::Incoming_Internal` validates the packet. Authorizes the connection's
/// `FInternetAddrBSD` on its `FSocketBSD` raw socket for 30 seconds.
#[cfg(windows)]
unsafe extern "system" fn hooked_received_packet(
    conn_ptr: *mut c_void,
    reader_ptr: *mut c_void,
    is_reinjected: u8,
) {
    let orig_addr: usize = ORIG_RECEIVED_PACKET.load(Ordering::Acquire);
    let module_base: usize = MODULE_BASE.load(Ordering::Acquire);

    if !conn_ptr.is_null() && module_base != 0 {
        // SAFETY: `conn_ptr` is a valid live `UNetConnection*`; read its primary vtable pointer
        // at `+0x00` to confirm it is a `UIpConnection` (or subclass) before reading IP fields.
        let conn_vtable: usize = unsafe { read_usize_at(conn_ptr, 0) };
        let is_ip_connection: bool = conn_vtable
            == module_base.wrapping_add(RVA_UIP_CONNECTION_VTABLE)
            || conn_vtable == module_base.wrapping_add(RVA_UNET_CONNECTION_EOS_VTABLE)
            || conn_vtable == module_base.wrapping_add(RVA_USTEAM_NET_CONNECTION_VTABLE);

        if is_ip_connection {
            // SAFETY: Verified `UIpConnection` instance; `+0x110` is `RemoteAddr.Object` and
            // `+0x1B58` is `SocketPrivate` (`FSocket*`).
            let remote_addr_ptr: *const c_void = unsafe {
                read_usize_at(conn_ptr, OFFSET_UNET_CONNECTION_REMOTE_ADDR) as *const c_void
            };
            // SAFETY: Verified `UIpConnection` instance; `+0x1B58` is `SocketPrivate`.
            let socket_ptr: *const c_void =
                unsafe { read_usize_at(conn_ptr, OFFSET_UIP_CONNECTION_SOCKET) as *const c_void };

            if !remote_addr_ptr.is_null() && !socket_ptr.is_null() {
                // SAFETY: Verify `remote_addr_ptr` is an `FInternetAddrBSD` and `socket_ptr` is
                // an `FSocketBSD` via their exact primary vtables before reading subclass fields.
                let addr_vtable: usize = unsafe { read_usize_at(remote_addr_ptr, 0) };
                // SAFETY: Read primary vtable of non-null `FSocket*`.
                let sock_vtable: usize = unsafe { read_usize_at(socket_ptr, 0) };
                let expected_addr_vt: usize =
                    module_base.wrapping_add(RVA_FINTERNET_ADDR_BSD_VTABLE);
                let expected_sock_vt: usize = module_base.wrapping_add(RVA_FSOCKET_BSD_VTABLE);

                if addr_vtable == expected_addr_vt && sock_vtable == expected_sock_vt {
                    // SAFETY: Verified `FSocketBSD*`.
                    let socket_type: u32 = unsafe { read_socket_type(socket_ptr) };
                    // SAFETY: Verified `FSocketBSD*`.
                    let raw_socket: usize = unsafe { read_raw_socket(socket_ptr) };

                    if socket_type == ESOCKET_TYPE_DATAGRAM && raw_socket != usize::MAX {
                        // SAFETY: Verified `FInternetAddrBSD*`.
                        if let Some(peer) = unsafe { extract_peer_endpoint(remote_addr_ptr) } {
                            // SAFETY: Lock-free monotonic tick query.
                            let now_ms: u64 = unsafe { GetTickCount64() };
                            if let Ok(mut guard) = relay_engine().lock() {
                                guard.record_authenticated_peer(raw_socket, peer, now_ms);
                            }
                        }
                    }
                }
            }
        }
    }

    if orig_addr != 0 {
        // SAFETY: `orig_addr` points to our sealed `RX` trampoline for `UNetConnection::ReceivedPacket`.
        let orig_fn: ReceivedPacketFn =
            unsafe { std::mem::transmute::<usize, ReceivedPacketFn>(orig_addr) };
        // SAFETY: Forwarding original call arguments untouched.
        unsafe { orig_fn(conn_ptr, reader_ptr, is_reinjected) };
    }
}

#[cfg(windows)]
unsafe extern "system" fn hooked_recv_from(
    socket_ptr: *mut c_void,
    data: *mut u8,
    buffer_size: i32,
    bytes_read: *mut i32,
    source: *mut c_void,
    flags: u32,
) -> u8 {
    let orig_addr: usize = ORIG_RECV_FROM.load(Ordering::Acquire);
    if orig_addr == 0 {
        return 0;
    }
    // SAFETY: `orig_addr` points to our sealed `RX` trampoline executing the verified
    // `FSocketBSD::RecvFrom` prologue and resuming the original Win64 ABI function.
    let orig_fn: RecvFromFn = unsafe { std::mem::transmute::<usize, RecvFromFn>(orig_addr) };

    if socket_ptr.is_null()
        || data.is_null()
        || bytes_read.is_null()
        || source.is_null()
        || buffer_size <= 0
    {
        // SAFETY: Forwarding invalid/edge arguments directly to original `FSocketBSD::RecvFrom`.
        return unsafe { orig_fn(socket_ptr, data, buffer_size, bytes_read, source, flags) };
    }

    // SAFETY: `socket_ptr` is non-null; verify `ESocketType == SOCKTYPE_Datagram` (UDP only).
    let socket_type: u32 = unsafe { read_socket_type(socket_ptr) };
    if socket_type != ESOCKET_TYPE_DATAGRAM {
        // SAFETY: Non-UDP sockets bypass voice demultiplexing untouched.
        return unsafe { orig_fn(socket_ptr, data, buffer_size, bytes_read, source, flags) };
    }

    // SAFETY: `socket_ptr` is a live UDP `FSocketBSD*`.
    let raw_socket: usize = unsafe { read_raw_socket(socket_ptr) };
    if raw_socket == usize::MAX {
        // SAFETY: Invalid socket handle delegates directly to UE4.
        return unsafe { orig_fn(socket_ptr, data, buffer_size, bytes_read, source, flags) };
    }

    if is_logging_enabled() && !FIRST_RECV_LOGGED.swap(true, Ordering::AcqRel) {
        log_line(&format!(
            "first UDP RecvFrom call: raw_socket={raw_socket} flags={flags}"
        ));
    }

    let is_peek: bool = (flags & ESOCKET_RECEIVE_FLAG_PEEK) != 0;

    for _iter in 0..MAX_DHV2_DRAIN_PER_CALL {
        // SAFETY: Invoking original `FSocketBSD::RecvFrom` with UE4's exact caller arguments.
        let ok: u8 = unsafe { orig_fn(socket_ptr, data, buffer_size, bytes_read, source, flags) };
        if ok == 0 {
            return 0;
        }

        // SAFETY: `bytes_read` is verified non-null and initialized by `orig_fn`.
        let mut read_len_i32: i32 = unsafe { *bytes_read };
        if read_len_i32 <= 0 {
            return ok;
        }

        let Ok(mut read_len): Result<usize, core::num::TryFromIntError> =
            usize::try_from(read_len_i32)
        else {
            return ok;
        };
        let Ok(max_buf): Result<usize, core::num::TryFromIntError> = usize::try_from(buffer_size)
        else {
            return ok;
        };
        if read_len > max_buf {
            return ok;
        }

        // SAFETY: `data` has at least `read_len <= buffer_size` initialized bytes from `orig_fn`.
        let initial_slice: &[u8] = unsafe { std::slice::from_raw_parts(data, read_len) };

        if !is_dhv2_packet(initial_slice) {
            // Normal UE4 game packet: return untouched to UE4. Peer authorization happens
            // STRICTLY post-handshake inside `UNetConnection::ReceivedPacket` (0x amplification).
            return ok;
        }

        // If UE4 passed `ESocketReceiveFlags::Peek`, consume the DHV2 datagram from the kernel
        // queue so we never spin on the same peeked voice packet.
        if is_peek {
            let pop_flags: u32 = flags & !ESOCKET_RECEIVE_FLAG_PEEK;
            // SAFETY: Draining the peeked DHV2 datagram with identical valid UE4 buffers.
            let pop_ok: u8 =
                unsafe { orig_fn(socket_ptr, data, buffer_size, bytes_read, source, pop_flags) };
            if pop_ok == 0 {
                return 0;
            }
            // SAFETY: `bytes_read` was populated by the consuming `orig_fn`.
            read_len_i32 = unsafe { *bytes_read };
            if read_len_i32 <= 0 {
                return pop_ok;
            }
            let Ok(popped_len): Result<usize, core::num::TryFromIntError> =
                usize::try_from(read_len_i32)
            else {
                return pop_ok;
            };
            if popped_len > max_buf {
                return pop_ok;
            }
            read_len = popped_len;

            // SAFETY: Re-verify the popped packet is still DHV2 in case of concurrent socket drain.
            let popped_check: &[u8] = unsafe { std::slice::from_raw_parts(data, read_len) };
            if !is_dhv2_packet(popped_check) {
                return pop_ok;
            }
        }

        // SAFETY: `source` holds the sender's `SOCKADDR_STORAGE` at `+0x08`.
        let sender_opt: Option<PeerEndpoint> = unsafe { extract_peer_endpoint(source) };
        if let Some(sender) = sender_opt {
            // SAFETY: `data` is exclusively owned by this `RecvFrom` invocation for `read_len` bytes.
            let packet_mut: &mut [u8] = unsafe { std::slice::from_raw_parts_mut(data, read_len) };
            // SAFETY: Lock-free monotonic tick query.
            let now_ms: u64 = unsafe { GetTickCount64() };

            let dispatch: RelayDispatch = match relay_engine().lock() {
                Ok(mut guard) => guard.handle_dhv2_packet(raw_socket, sender, packet_mut, now_ms),
                Err(_poisoned) => RelayDispatch::Drop,
            };

            // Lock is released BEFORE any WinSock `sendto` syscalls!
            match dispatch {
                RelayDispatch::Drop => {
                    if is_logging_enabled()
                        && !FIRST_UNAUTH_DROP_LOGGED.swap(true, Ordering::AcqRel)
                    {
                        log_line(&format!(
                            "silently dropped unauthenticated/rejected DHV2 packet ({read_len}B)"
                        ));
                    }
                }
                RelayDispatch::SendControl(packets) => {
                    for pkt in packets {
                        send_control_packet(raw_socket, &pkt);
                    }
                }
                RelayDispatch::RelayVoice {
                    recipients,
                    control_packets,
                } => {
                    for recipient in recipients {
                        send_udp_packet(raw_socket, recipient, packet_mut);
                    }
                    for pkt in control_packets {
                        send_control_packet(raw_socket, &pkt);
                    }
                }
            }
        }
    }

    // All `MAX_DHV2_DRAIN_PER_CALL` slots were DHV2 packets: tell UE4 "no game packet ready"
    // (`*BytesRead = 0`, return `true`) so UE4 never sees DHV2 bytes.
    // SAFETY: `bytes_read` is verified non-null.
    unsafe {
        *bytes_read = 0;
    }
    1
}

#[cfg(windows)]
fn send_control_packet(raw_socket: usize, pkt: &OutboundControlPacket) {
    send_udp_packet(raw_socket, pkt.target, &pkt.payload);
}

#[cfg(windows)]
unsafe extern "system" fn hooked_close(socket_ptr: *mut c_void) -> u8 {
    let orig_addr: usize = ORIG_CLOSE.load(Ordering::Acquire);
    if orig_addr == 0 {
        return 0;
    }

    if !socket_ptr.is_null() {
        // SAFETY: Read `FSocketBSD::Socket` BEFORE `orig_close` resets it to `INVALID_SOCKET`.
        let raw_socket: usize = unsafe { read_raw_socket(socket_ptr) };
        if raw_socket != usize::MAX {
            if let Ok(mut guard) = relay_engine().lock() {
                guard.on_socket_closed(raw_socket);
            }
        }
    }

    // SAFETY: `orig_addr` points to our sealed `RX` trampoline for `FSocketBSD::Close`.
    let orig_fn: CloseFn = unsafe { std::mem::transmute::<usize, CloseFn>(orig_addr) };
    // SAFETY: Forwarding call directly to original `FSocketBSD::Close`.
    unsafe { orig_fn(socket_ptr) }
}
