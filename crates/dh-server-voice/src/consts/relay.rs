//! Relay state machine, anti-spoofing, and anti-amplification rate-limiting constants.

/// Maximum consecutive `DHV2` packets drained inside a single `FSocketBSD::RecvFrom` call.
pub const MAX_DHV2_DRAIN_PER_CALL: usize = 128;

/// Maximum entries tracked in the post-handshake authorized game-peer table per UDP socket.
pub const MAX_AUTHORIZED_GAME_PEERS: usize = 64;

/// Maximum concurrent voice peers allowed on the server relay (Dread Hunger is an 8-player game;
/// caps max single-frame fanout strictly to `<= 15`).
pub const MAX_VOICE_PEERS: usize = 16;

/// Maximum server-assigned peer slot (inclusive, `1..=16`).
pub const MAX_SERVER_PEER_SLOT: u16 = 16;

/// Maximum Opus voice frame payload bytes accepted and relayed by the server (`<= 256` B;
/// 32 kbps / 20 ms VOIP + FEC averages 60–110 B).
pub const MAX_SERVER_VOICE_PAYLOAD: usize = 256;

/// Maximum allowed UTF-8 byte length of a `Hello` peer UID on the server relay.
pub const MAX_SERVER_UID_LEN: usize = 96;

/// Minimum printable non-whitespace ASCII byte permitted in a peer UID (`'!'`).
pub const UID_ASCII_MIN: u8 = 0x21;

/// Maximum printable non-whitespace ASCII byte permitted in a peer UID (`'~'`).
pub const UID_ASCII_MAX: u8 = 0x7E;

/// Authorization TTL in milliseconds: an endpoint must have passed UE4 post-handshake
/// `UNetConnection::ReceivedPacket` within this window (30 seconds).
pub const GAME_PEER_AUTH_TTL_MS: u64 = 30_000;

/// Voice peer inactivity timeout in milliseconds: peers silent longer than this are evicted
/// and a fresh `PeerMap` is broadcast (15 seconds).
pub const VOICE_PEER_TIMEOUT_MS: u64 = 15_000;

/// Minimum interval in milliseconds between periodic stale-peer cleanup sweeps per socket.
pub const STALE_SWEEP_INTERVAL_MS: u64 = 1_000;

/// Fixed-point millitoken scale factor (`1 token = 1000 millitokens`).
pub const TOKEN_MILLI_SCALE: u64 = 1_000;

/// Per-peer `VoiceFrame` token bucket steady refill rate (packets/sec; normal 20 ms speech = 50 pps).
pub const VOICE_RATE_PER_SEC: u64 = 65;

/// Per-peer `VoiceFrame` token bucket burst capacity (packets).
pub const VOICE_BURST_PACKETS: u64 = 15;

/// Per-peer `Hello` token bucket steady refill rate (packets/sec; prevents `PeerMap` spam).
pub const HELLO_RATE_PER_SEC: u64 = 1;

/// Per-peer `Hello` token bucket burst capacity (packets).
pub const HELLO_BURST_PACKETS: u64 = 2;

/// Global per-socket outbound voice relay cap (outbound datagrams/sec across all peers;
/// 8 simultaneous talkers × 50 pps × 7 recipients = 2800 pps).
pub const GLOBAL_RELAY_RATE_PER_SEC: u64 = 3_000;

/// Global per-socket outbound voice relay burst capacity (outbound datagrams).
pub const GLOBAL_RELAY_BURST_PACKETS: u64 = 300;
