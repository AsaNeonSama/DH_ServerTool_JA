//! Types for the hardened zero-extra-port DHV2 UDP server voice relay state machine.

use std::collections::HashMap;

/// Normalized UDP peer endpoint supporting both IPv4 (`AF_INET`) and IPv6 (`AF_INET6`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeerEndpoint {
    /// IPv4 endpoint (`addr` in network byte order, `port` in host byte order).
    V4 {
        /// 4-byte IPv4 address.
        addr: [u8; 4],
        /// UDP port in host byte order.
        port: u16,
    },
    /// IPv6 endpoint (`addr`, `port` in host byte order, raw `flowinfo` and `scope_id`).
    V6 {
        /// 16-byte IPv6 address.
        addr: [u8; 16],
        /// UDP port in host byte order.
        port: u16,
        /// IPv6 flow info bytes (raw `WinSock` representation).
        flowinfo: [u8; 4],
        /// IPv6 scope ID bytes (raw `WinSock` representation).
        scope_id: [u8; 4],
    },
}

/// 8-byte aligned raw `SOCKADDR_IN` / `SOCKADDR_IN6` storage buffer for `WinSock` `sendto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C, align(8))]
pub struct AlignedSockAddr {
    /// Raw `SOCKADDR_IN` (first 16 bytes) or `SOCKADDR_IN6` (28 bytes) bytes.
    pub bytes: [u8; 28],
}

/// Integer fixed-point millitoken bucket (`1 token = 1000 millitokens`) for deterministic rate limiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenBucket {
    /// Available balance in millitokens.
    pub tokens_milli: u64,
    /// Last monotonic refill timestamp in milliseconds.
    pub last_refill_ms: u64,
}

/// An active voice peer registered on a server UDP socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoicePeerEntry {
    /// Server-assigned authoritative peer slot (`1..=16`).
    pub peer_slot: u16,
    /// Peer's UDP socket endpoint (`IP:Port`).
    pub endpoint: PeerEndpoint,
    /// Player identifier string (printable ASCII `1..=96` bytes).
    pub uid: String,
    /// Last monotonic timestamp (ms) when a `Hello` or `VoiceFrame` was received.
    pub last_seen_ms: u64,
    /// Per-peer `VoiceFrame` rate limiter (65 pps steady, burst 15).
    pub voice_limiter: TokenBucket,
}

/// Per-socket relay table tracking post-handshake UE4 game endpoints, rate limiters, and voice peers.
#[derive(Debug, Default)]
pub struct SocketRelayState {
    /// Endpoints authenticated via UE4 post-handshake `UNetConnection::ReceivedPacket` (`endpoint -> last_seen_ms`).
    pub authorized_game_peers: HashMap<PeerEndpoint, u64>,
    /// Per-endpoint `Hello` rate limiters (`1/s`, burst `2`), persisted across `Bye` to prevent cycling spam.
    pub hello_limiters: HashMap<PeerEndpoint, TokenBucket>,
    /// Active voice peers indexed by `endpoint` (strictly capped at `MAX_VOICE_PEERS = 16`).
    pub voice_peers: HashMap<PeerEndpoint, VoicePeerEntry>,
    /// Global outbound voice relay token bucket (`3000` outbound packets/sec across all peers).
    pub global_voice_limiter: Option<TokenBucket>,
    /// Last monotonic timestamp (ms) when stale peers were swept.
    pub last_sweep_ms: u64,
}

/// Root relay state holding per-socket tables keyed by raw Win32 `SOCKET` handle.
#[derive(Debug, Default)]
pub struct RelayEngine {
    /// Active UDP socket states keyed by raw `SOCKET` handle (`usize`).
    pub sockets: HashMap<usize, SocketRelayState>,
}

/// A serialized control datagram to transmit to a specific peer endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundControlPacket {
    /// Destination peer endpoint.
    pub target: PeerEndpoint,
    /// Complete wire-ready DHV2 packet bytes.
    pub payload: Vec<u8>,
}

/// Action returned by [`RelayEngine::handle_dhv2_packet`] to be executed after releasing locks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayDispatch {
    /// Packet dropped (unauthorized endpoint, malformed/oversized frame, rate-limited, or unregistered sender).
    Drop,
    /// Send one or more control packets (`HelloAck`, `PeerMap`) to specified destinations.
    SendControl(Vec<OutboundControlPacket>),
    /// Forward the in-place patched `VoiceFrame` buffer immediately to every listed recipient,
    /// plus any optional control broadcasts (e.g. `PeerMap` after stale-peer eviction).
    RelayVoice {
        /// Recipient endpoints (`!= sender`).
        recipients: Vec<PeerEndpoint>,
        /// Optional control packets generated during stale cleanup.
        control_packets: Vec<OutboundControlPacket>,
    },
}
