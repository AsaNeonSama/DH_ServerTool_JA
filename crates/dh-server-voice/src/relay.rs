//! Hardened pure-Rust DHV2 zero-extra-port server voice relay state machine.

use dh_voice_proto::consts::{
    DHV2_HEADER_LEN, MAX_PACKET_SIZE, MIN_PEER_SLOT, PEER_MAP_ENTRY_OVERHEAD,
};
use dh_voice_proto::{
    decode_header, decode_message, encode_hello_ack, encode_peer_map, patch_voice_frame_peer_slot,
    Dhv2Header, Dhv2Message, MsgType, PeerMapEntry,
};

use crate::consts::patch::{
    ADDR_FAMILY_INET, ADDR_FAMILY_INET6, SOCKADDR_IN6_LEN, SOCKADDR_IN_LEN,
};
use crate::consts::relay::{
    GAME_PEER_AUTH_TTL_MS, GLOBAL_RELAY_BURST_PACKETS, GLOBAL_RELAY_RATE_PER_SEC,
    HELLO_BURST_PACKETS, HELLO_RATE_PER_SEC, MAX_AUTHORIZED_GAME_PEERS, MAX_SERVER_PEER_SLOT,
    MAX_SERVER_UID_LEN, MAX_SERVER_VOICE_PAYLOAD, MAX_VOICE_PEERS, STALE_SWEEP_INTERVAL_MS,
    TOKEN_MILLI_SCALE, UID_ASCII_MAX, UID_ASCII_MIN, VOICE_BURST_PACKETS, VOICE_PEER_TIMEOUT_MS,
    VOICE_RATE_PER_SEC,
};
use crate::structs::relay::{
    OutboundControlPacket, PeerEndpoint, RelayDispatch, RelayEngine, SocketRelayState, TokenBucket,
    VoicePeerEntry,
};

impl TokenBucket {
    /// Creates a full token bucket initialized at `now_ms` with `burst` tokens.
    #[must_use]
    pub const fn new_full(burst: u64, now_ms: u64) -> Self {
        Self {
            tokens_milli: burst.saturating_mul(TOKEN_MILLI_SCALE),
            last_refill_ms: now_ms,
        }
    }

    /// Refills the bucket up to `burst` tokens at `rate_per_sec` and attempts to consume `cost` tokens.
    ///
    /// Uses integer millitokens (`1 token = 1000 millitokens`), so `elapsed_ms * rate_per_sec`
    /// equals exact millitokens earned without floating-point drift.
    pub fn try_consume(&mut self, cost: u64, now_ms: u64, rate_per_sec: u64, burst: u64) -> bool {
        let elapsed_ms: u64 = now_ms.saturating_sub(self.last_refill_ms);
        let max_milli: u64 = burst.saturating_mul(TOKEN_MILLI_SCALE);
        let earned_milli: u64 = elapsed_ms.saturating_mul(rate_per_sec);
        self.tokens_milli = self
            .tokens_milli
            .saturating_add(earned_milli)
            .min(max_milli);
        if now_ms > self.last_refill_ms {
            self.last_refill_ms = now_ms;
        }

        let cost_milli: u64 = cost.saturating_mul(TOKEN_MILLI_SCALE);
        if self.tokens_milli >= cost_milli {
            self.tokens_milli -= cost_milli;
            true
        } else {
            false
        }
    }
}

/// Validates a `Hello` peer UID against strict server-side rules:
/// non-empty, `<= 96` bytes, printable non-whitespace ASCII (`0x21..=0x7E`) only.
#[must_use]
pub fn is_valid_server_uid(uid: &str) -> bool {
    !uid.is_empty()
        && uid.len() <= MAX_SERVER_UID_LEN
        && uid
            .bytes()
            .all(|byte: u8| (UID_ASCII_MIN..=UID_ASCII_MAX).contains(&byte))
}

impl PeerEndpoint {
    /// Parses a normalized [`PeerEndpoint`] from a raw `WinSock` `SOCKADDR_STORAGE` byte slice.
    ///
    /// Supports both IPv4 (`AF_INET`, 16 bytes) and IPv6 (`AF_INET6`, 28 bytes).
    #[must_use]
    pub fn from_sockaddr_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 2 {
            return None;
        }
        let family: u16 = u16::from_ne_bytes([bytes[0], bytes[1]]);
        if family == ADDR_FAMILY_INET {
            let min_len: usize = usize::try_from(SOCKADDR_IN_LEN).ok()?;
            if bytes.len() < min_len {
                return None;
            }
            let port: u16 = u16::from_be_bytes([bytes[2], bytes[3]]);
            if port == 0 {
                return None;
            }
            let addr: [u8; 4] = [bytes[4], bytes[5], bytes[6], bytes[7]];
            Some(Self::V4 { addr, port })
        } else if family == ADDR_FAMILY_INET6 {
            let min_len: usize = usize::try_from(SOCKADDR_IN6_LEN).ok()?;
            if bytes.len() < min_len {
                return None;
            }
            let port: u16 = u16::from_be_bytes([bytes[2], bytes[3]]);
            if port == 0 {
                return None;
            }
            let flowinfo: [u8; 4] = [bytes[4], bytes[5], bytes[6], bytes[7]];
            let mut addr: [u8; 16] = [0u8; 16];
            addr.copy_from_slice(&bytes[8..24]);
            let scope_id: [u8; 4] = [bytes[24], bytes[25], bytes[26], bytes[27]];
            Some(Self::V6 {
                addr,
                port,
                flowinfo,
                scope_id,
            })
        } else {
            None
        }
    }

    /// Serializes this endpoint into an exact `WinSock` `SOCKADDR_IN` (16B) or `SOCKADDR_IN6` (28B)
    /// byte buffer and returns `(buffer, addr_len)`.
    #[must_use]
    pub fn to_sockaddr_bytes(self) -> ([u8; 28], i32) {
        let mut raw: [u8; 28] = [0u8; 28];
        match self {
            Self::V4 { addr, port } => {
                raw[0..2].copy_from_slice(&ADDR_FAMILY_INET.to_ne_bytes());
                raw[2..4].copy_from_slice(&port.to_be_bytes());
                raw[4..8].copy_from_slice(&addr);
                (raw, SOCKADDR_IN_LEN)
            }
            Self::V6 {
                addr,
                port,
                flowinfo,
                scope_id,
            } => {
                raw[0..2].copy_from_slice(&ADDR_FAMILY_INET6.to_ne_bytes());
                raw[2..4].copy_from_slice(&port.to_be_bytes());
                raw[4..8].copy_from_slice(&flowinfo);
                raw[8..24].copy_from_slice(&addr);
                raw[24..28].copy_from_slice(&scope_id);
                (raw, SOCKADDR_IN6_LEN)
            }
        }
    }
}

impl RelayEngine {
    /// Creates an empty relay engine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records or refreshes a post-handshake authenticated UE4 `UIpConnection` peer endpoint
    /// in the 30-second authorization table.
    ///
    /// Called EXCLUSIVELY from `UNetConnection::ReceivedPacket` (never from raw `RecvFrom`),
    /// enforcing `MAX_AUTHORIZED_GAME_PEERS` (`64`) per socket via expiry pruning + LRU eviction.
    pub fn record_authenticated_peer(
        &mut self,
        socket_handle: usize,
        peer: PeerEndpoint,
        now_ms: u64,
    ) {
        let state: &mut SocketRelayState = self.sockets.entry(socket_handle).or_default();
        if let Some(seen) = state.authorized_game_peers.get_mut(&peer) {
            *seen = now_ms;
            return;
        }

        if state.authorized_game_peers.len() >= MAX_AUTHORIZED_GAME_PEERS {
            state
                .authorized_game_peers
                .retain(|_ep: &PeerEndpoint, seen: &mut u64| {
                    now_ms.saturating_sub(*seen) <= GAME_PEER_AUTH_TTL_MS
                });
            state
                .hello_limiters
                .retain(|ep: &PeerEndpoint, _b: &mut TokenBucket| {
                    state.authorized_game_peers.contains_key(ep)
                });
        }

        if state.authorized_game_peers.len() >= MAX_AUTHORIZED_GAME_PEERS {
            let oldest: Option<PeerEndpoint> = state
                .authorized_game_peers
                .iter()
                .min_by_key(|(_ep, seen): &(&PeerEndpoint, &u64)| **seen)
                .map(|(ep, _seen): (&PeerEndpoint, &u64)| *ep);
            if let Some(evict_ep) = oldest {
                state.authorized_game_peers.remove(&evict_ep);
                state.hello_limiters.remove(&evict_ep);
            }
        }

        state.authorized_game_peers.insert(peer, now_ms);
    }

    /// Processes an incoming `DHV2` datagram on `socket_handle` from `sender`.
    ///
    /// Rejects any endpoint that has not completed UE4's post-handshake `UNetConnection::ReceivedPacket`
    /// within the last 30s (`0x` amplification against spoofed IPs), enforces per-peer & global
    /// token-bucket rate limits, enforces `<= 16` voice peers, `<= 256` B Opus payload, and strict
    /// printable-ASCII `<= 96` B UIDs.
    #[must_use]
    pub fn handle_dhv2_packet(
        &mut self,
        socket_handle: usize,
        sender: PeerEndpoint,
        packet_buf: &mut [u8],
        now_ms: u64,
    ) -> RelayDispatch {
        let Some(state): Option<&mut SocketRelayState> = self.sockets.get_mut(&socket_handle)
        else {
            return RelayDispatch::Drop;
        };

        if !is_authorized_game_peer(state, sender, now_ms) {
            return RelayDispatch::Drop;
        }

        let evicted: bool = sweep_stale_peers(state, now_ms, false);

        let Ok(header): Result<Dhv2Header, dh_voice_proto::ProtoError> = decode_header(packet_buf)
        else {
            return eviction_control_or_drop(state, evicted);
        };

        match header.msg_type {
            MsgType::VoiceFrame => handle_voice_frame(
                state,
                sender,
                packet_buf,
                header.payload_len,
                now_ms,
                evicted,
            ),
            MsgType::Hello => handle_hello(state, sender, packet_buf, now_ms, evicted),
            MsgType::Bye => handle_bye(state, sender, packet_buf, evicted),
            MsgType::HelloAck | MsgType::PeerMap => eviction_control_or_drop(state, evicted),
        }
    }

    /// Removes all authorized game peers, rate limiters, and active voice peers for a closed socket.
    pub fn on_socket_closed(&mut self, socket_handle: usize) {
        self.sockets.remove(&socket_handle);
    }

    /// Returns the number of active voice peers registered on `socket_handle`.
    #[must_use]
    pub fn active_voice_peer_count(&self, socket_handle: usize) -> usize {
        self.sockets
            .get(&socket_handle)
            .map_or(0, |state: &SocketRelayState| state.voice_peers.len())
    }
}

fn is_authorized_game_peer(state: &SocketRelayState, peer: PeerEndpoint, now_ms: u64) -> bool {
    state
        .authorized_game_peers
        .get(&peer)
        .is_some_and(|seen: &u64| now_ms.saturating_sub(*seen) <= GAME_PEER_AUTH_TTL_MS)
}

fn sweep_stale_peers(state: &mut SocketRelayState, now_ms: u64, force: bool) -> bool {
    if !force && now_ms.saturating_sub(state.last_sweep_ms) < STALE_SWEEP_INTERVAL_MS {
        return false;
    }
    state.last_sweep_ms = now_ms;

    state
        .authorized_game_peers
        .retain(|_ep: &PeerEndpoint, seen: &mut u64| {
            now_ms.saturating_sub(*seen) <= GAME_PEER_AUTH_TTL_MS
        });
    state
        .hello_limiters
        .retain(|ep: &PeerEndpoint, _b: &mut TokenBucket| {
            state.authorized_game_peers.contains_key(ep)
        });

    let before_count: usize = state.voice_peers.len();
    state
        .voice_peers
        .retain(|_ep: &PeerEndpoint, entry: &mut VoicePeerEntry| {
            now_ms.saturating_sub(entry.last_seen_ms) <= VOICE_PEER_TIMEOUT_MS
        });
    state.voice_peers.len() != before_count
}

fn allocate_free_slot(state: &SocketRelayState) -> Option<u16> {
    if state.voice_peers.len() >= MAX_VOICE_PEERS {
        return None;
    }
    for candidate in MIN_PEER_SLOT..=MAX_SERVER_PEER_SLOT {
        let in_use: bool = state
            .voice_peers
            .values()
            .any(|entry: &VoicePeerEntry| entry.peer_slot == candidate);
        if !in_use {
            return Some(candidate);
        }
    }
    None
}

fn build_peer_map_bytes(state: &SocketRelayState) -> Option<Vec<u8>> {
    let mut ordered: Vec<&VoicePeerEntry> = state.voice_peers.values().collect();
    ordered.sort_by_key(|entry: &&VoicePeerEntry| entry.peer_slot);

    let mut entries: Vec<PeerMapEntry> = Vec::with_capacity(ordered.len());
    let mut byte_budget: usize = DHV2_HEADER_LEN + 1;
    for peer in ordered {
        let needed: usize = PEER_MAP_ENTRY_OVERHEAD + peer.uid.len();
        if byte_budget.saturating_add(needed) > MAX_PACKET_SIZE {
            break;
        }
        byte_budget += needed;
        entries.push(PeerMapEntry {
            peer_slot: peer.peer_slot,
            uid: peer.uid.clone(),
        });
    }

    encode_peer_map(&entries).ok()
}

fn build_peer_map_broadcast(state: &SocketRelayState) -> Vec<OutboundControlPacket> {
    if state.voice_peers.is_empty() {
        return Vec::new();
    }
    let Some(map_bytes): Option<Vec<u8>> = build_peer_map_bytes(state) else {
        return Vec::new();
    };
    let mut targets: Vec<PeerEndpoint> = state.voice_peers.keys().copied().collect();
    targets.sort_by_key(|ep: &PeerEndpoint| {
        state
            .voice_peers
            .get(ep)
            .map_or(u16::MAX, |peer: &VoicePeerEntry| peer.peer_slot)
    });

    targets
        .into_iter()
        .map(|target: PeerEndpoint| OutboundControlPacket {
            target,
            payload: map_bytes.clone(),
        })
        .collect()
}

fn eviction_control_or_drop(state: &SocketRelayState, evicted: bool) -> RelayDispatch {
    if evicted {
        let broadcasts: Vec<OutboundControlPacket> = build_peer_map_broadcast(state);
        if !broadcasts.is_empty() {
            return RelayDispatch::SendControl(broadcasts);
        }
    }
    RelayDispatch::Drop
}

fn handle_voice_frame(
    state: &mut SocketRelayState,
    sender: PeerEndpoint,
    packet_buf: &mut [u8],
    payload_len: u16,
    now_ms: u64,
    evicted: bool,
) -> RelayDispatch {
    if usize::from(payload_len) > MAX_SERVER_VOICE_PAYLOAD {
        return eviction_control_or_drop(state, evicted);
    }

    let Some(sender_entry): Option<&mut VoicePeerEntry> = state.voice_peers.get_mut(&sender) else {
        return eviction_control_or_drop(state, evicted);
    };
    sender_entry.last_seen_ms = now_ms;

    if !sender_entry
        .voice_limiter
        .try_consume(1, now_ms, VOICE_RATE_PER_SEC, VOICE_BURST_PACKETS)
    {
        return eviction_control_or_drop(state, evicted);
    }

    let authoritative_slot: u16 = sender_entry.peer_slot;
    if !patch_voice_frame_peer_slot(packet_buf, authoritative_slot) {
        return eviction_control_or_drop(state, evicted);
    }

    let recipients: Vec<PeerEndpoint> = state
        .voice_peers
        .keys()
        .copied()
        .filter(|ep: &PeerEndpoint| *ep != sender)
        .collect();

    if !recipients.is_empty() {
        let cost: u64 = u64::try_from(recipients.len()).unwrap_or(u64::MAX);
        let global_bucket: &mut TokenBucket = state
            .global_voice_limiter
            .get_or_insert_with(|| TokenBucket::new_full(GLOBAL_RELAY_BURST_PACKETS, now_ms));
        if !global_bucket.try_consume(
            cost,
            now_ms,
            GLOBAL_RELAY_RATE_PER_SEC,
            GLOBAL_RELAY_BURST_PACKETS,
        ) {
            return eviction_control_or_drop(state, evicted);
        }
    }

    let control_packets: Vec<OutboundControlPacket> = if evicted {
        build_peer_map_broadcast(state)
    } else {
        Vec::new()
    };

    if recipients.is_empty() && control_packets.is_empty() {
        RelayDispatch::Drop
    } else {
        RelayDispatch::RelayVoice {
            recipients,
            control_packets,
        }
    }
}

fn handle_hello(
    state: &mut SocketRelayState,
    sender: PeerEndpoint,
    packet_buf: &[u8],
    now_ms: u64,
    evicted: bool,
) -> RelayDispatch {
    let Ok(Dhv2Message::Hello { uid }): Result<Dhv2Message, dh_voice_proto::ProtoError> =
        decode_message(packet_buf)
    else {
        return eviction_control_or_drop(state, evicted);
    };

    if !is_valid_server_uid(&uid) {
        return eviction_control_or_drop(state, evicted);
    }

    let hello_bucket: &mut TokenBucket = state
        .hello_limiters
        .entry(sender)
        .or_insert_with(|| TokenBucket::new_full(HELLO_BURST_PACKETS, now_ms));
    if !hello_bucket.try_consume(1, now_ms, HELLO_RATE_PER_SEC, HELLO_BURST_PACKETS) {
        return eviction_control_or_drop(state, evicted);
    }

    let mut roster_changed: bool = evicted;
    let assigned_slot: u16 = if let Some(existing) = state.voice_peers.get_mut(&sender) {
        existing.last_seen_ms = now_ms;
        if existing.uid != uid {
            existing.uid = uid;
            roster_changed = true;
        }
        existing.peer_slot
    } else {
        let existing_same_uid: Option<(PeerEndpoint, u16)> = state
            .voice_peers
            .iter()
            .find(|(_ep, entry): &(&PeerEndpoint, &VoicePeerEntry)| entry.uid == uid)
            .map(|(ep, entry): (&PeerEndpoint, &VoicePeerEntry)| (*ep, entry.peer_slot));

        let slot: u16 = if let Some((old_ep, reused_slot)) = existing_same_uid {
            state.voice_peers.remove(&old_ep);
            reused_slot
        } else {
            let first_try: Option<u16> = allocate_free_slot(state);
            if let Some(free_slot) = first_try {
                free_slot
            } else {
                let forced_eviction: bool = sweep_stale_peers(state, now_ms, true);
                if forced_eviction {
                    roster_changed = true;
                }
                let Some(retry_slot): Option<u16> = allocate_free_slot(state) else {
                    return eviction_control_or_drop(state, roster_changed);
                };
                retry_slot
            }
        };

        state.voice_peers.insert(
            sender,
            VoicePeerEntry {
                peer_slot: slot,
                endpoint: sender,
                uid,
                last_seen_ms: now_ms,
                voice_limiter: TokenBucket::new_full(VOICE_BURST_PACKETS, now_ms),
            },
        );
        roster_changed = true;
        slot
    };

    let mut outbound: Vec<OutboundControlPacket> = Vec::new();
    if let Ok(ack_bytes) = encode_hello_ack(assigned_slot) {
        outbound.push(OutboundControlPacket {
            target: sender,
            payload: ack_bytes,
        });
    }

    if roster_changed {
        outbound.extend(build_peer_map_broadcast(state));
    } else if let Some(map_bytes) = build_peer_map_bytes(state) {
        outbound.push(OutboundControlPacket {
            target: sender,
            payload: map_bytes,
        });
    }

    if outbound.is_empty() {
        RelayDispatch::Drop
    } else {
        RelayDispatch::SendControl(outbound)
    }
}

fn handle_bye(
    state: &mut SocketRelayState,
    sender: PeerEndpoint,
    packet_buf: &[u8],
    evicted: bool,
) -> RelayDispatch {
    let Ok(Dhv2Message::Bye { .. }): Result<Dhv2Message, dh_voice_proto::ProtoError> =
        decode_message(packet_buf)
    else {
        return eviction_control_or_drop(state, evicted);
    };

    let removed: bool = state.voice_peers.remove(&sender).is_some();
    eviction_control_or_drop(state, removed || evicted)
}
