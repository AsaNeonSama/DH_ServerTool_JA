use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use dh_voice_proto::{
    decode_message, encode_bye_into, encode_hello_into, encode_voice_frame_into, Dhv2Message,
    PeerMapEntry, FLAG_END_OF_TALKSPURT, FLAG_HAS_FEC, MAX_OPUS_PAYLOAD, MAX_PACKET_SIZE,
    MAX_PEER_SLOT, MIN_PEER_SLOT,
};

use crate::consts::dsp::{DSP_FRAME_SAMPLES_10MS, OPUS_FRAME_SAMPLES_20MS};
use crate::consts::engine::{
    HELLO_KEEPALIVE_INTERVAL_MS, HELLO_RETRY_INTERVAL_MS, INVALID_RAW_SOCKET, SOCKADDR_AF_INET,
    SOCKADDR_AF_INET6, SOCKADDR_INET6_LEN, SOCKADDR_INET_LEN, SOCKADDR_STORAGE_SIZE,
};
use crate::consts::jitter::JITTER_MAX_SPEAKERS;
use crate::consts::opus::OPUS_DTX_SILENCE_MAX_BYTES;
use crate::consts::ue4::OFFSET_FINTERNET_ADDR_BSD_STORAGE;
use crate::dsp::{normalized_f32_to_i16, DspPipeline};
use crate::jitter::{advance_playout_accumulator, plc_fade_gain, SpeakerJitterBuffer};
use crate::logger::log_line;
use crate::structs::engine::{
    ClientVoiceSession, DecodedPlayoutFrame, GameSocketEndpoint, RemotePeerVoice,
    TxCapturePipeline, VoiceRxState,
};
use crate::structs::jitter::PlayoutAction;
use crate::structs::opus::{OpusApiTable, OpusDecoder, OpusEncoder};
use crate::structs::wasapi::{CaptureFrameFactory, CaptureFrameHandler, WasapiSession};

#[cfg(windows)]
use windows::Win32::Networking::WinSock::{sendto, SOCKADDR, SOCKADDR_STORAGE, SOCKET};

/// Current `FSocketBSD*` owned by `UIpNetDriver::GetSocket()` while in a Dedicated Server match.
static MATCH_GAME_SOCKET_PTR: AtomicUsize = AtomicUsize::new(0);

/// Lock-free mirror of the currently cached `GameSocketEndpoint::socket_ptr` (`0` if unset).
static CACHED_ENDPOINT_SOCKET_PTR: AtomicUsize = AtomicUsize::new(0);

/// Active local transmit gate synchronized with `UDH_OnlineVoice::bWantsToTalk` (`+0x80`).
static LOCAL_WANTS_TO_TALK: AtomicBool = AtomicBool::new(false);

/// Server-assigned authoritative `peer_slot` (`1..=64`, or `0` prior to `HelloAck`).
static ASSIGNED_PEER_SLOT: AtomicU16 = AtomicU16::new(0);

/// Active UDP socket handle + destination `SOCKADDR_STORAGE` protected against concurrent `Close`.
static GAME_ENDPOINT: Mutex<Option<GameSocketEndpoint>> = Mutex::new(None);

/// Lazy-initialized receive state shared between `FSocketBSD::RecvFrom` and `FEngineLoop::Tick`.
static VOICE_RX_STATE: OnceLock<Mutex<VoiceRxState>> = OnceLock::new();

/// Lazy-initialized match session state owned by the UE4 Game Thread (`FEngineLoop::Tick`).
static VOICE_SESSION: OnceLock<Mutex<ClientVoiceSession>> = OnceLock::new();

fn rx_state() -> &'static Mutex<VoiceRxState> {
    VOICE_RX_STATE.get_or_init(|| Mutex::new(VoiceRxState::new(None)))
}

fn session_state() -> &'static Mutex<ClientVoiceSession> {
    VOICE_SESSION.get_or_init(|| {
        Mutex::new(ClientVoiceSession {
            wasapi: None,
            local_uid: None,
            last_hello_sent: None,
            logged_ack_slot: 0,
        })
    })
}

/// Records the active Dedicated Server `UIpNetDriver::GetSocket()` pointer (`0` when outside match).
pub fn set_match_game_socket_ptr(socket_ptr: usize) {
    MATCH_GAME_SOCKET_PTR.store(socket_ptr, Ordering::Release);
}

/// Updates the lock-free local PTT / Toggle-to-Talk gate read by the WASAPI capture TX thread.
pub fn set_local_wants_to_talk(wants: bool) {
    LOCAL_WANTS_TO_TALK.store(wants, Ordering::Release);
}

/// Returns the currently assigned server `peer_slot` (`0` before `HelloAck`).
#[must_use]
pub fn assigned_peer_slot() -> u16 {
    ASSIGNED_PEER_SLOT.load(Ordering::Acquire)
}

/// Encodes a UTF-8 player UID into a NUL-terminated UTF-16LE buffer suitable for read-only
/// UE4 `FString` parameters.
#[must_use]
pub fn encode_uid_utf16_nul(uid: &str) -> Vec<u16> {
    let mut out: Vec<u16> = uid.encode_utf16().collect();
    out.push(0u16);
    out
}

/// Applies smooth PLC fade-out attenuation (`[1.0, 0.70, 0.35]`) to a synthesized `20 ms` PCM frame.
pub fn apply_plc_fade_in_place(pcm: &mut [i16; OPUS_FRAME_SAMPLES_20MS], loss_streak: u32) {
    let gain: f32 = plc_fade_gain(loss_streak);
    if gain >= 0.999 {
        return;
    }
    for sample in pcm.iter_mut() {
        let norm: f32 = (f32::from(*sample) / 32_768.0_f32) * gain;
        *sample = normalized_f32_to_i16(norm);
    }
}

/// Validates and normalizes a raw `SOCKADDR_STORAGE` buffer, returning `(normalized_bytes, len)`.
#[must_use]
pub fn parse_sockaddr_storage(
    raw: &[u8; SOCKADDR_STORAGE_SIZE],
) -> Option<([u8; SOCKADDR_STORAGE_SIZE], i32)> {
    let family: u16 = u16::from_le_bytes([raw[0], raw[1]]);
    let port_be: u16 = u16::from_be_bytes([raw[2], raw[3]]);
    if port_be == 0 {
        return None;
    }
    let addr_len: i32 = match family {
        SOCKADDR_AF_INET => SOCKADDR_INET_LEN,
        SOCKADDR_AF_INET6 => SOCKADDR_INET6_LEN,
        _ => return None,
    };
    let len_usize: usize = usize::try_from(addr_len).ok()?;
    let mut normalized: [u8; SOCKADDR_STORAGE_SIZE] = [0u8; SOCKADDR_STORAGE_SIZE];
    normalized[..len_usize].copy_from_slice(&raw[..len_usize]);
    Some((normalized, addr_len))
}

/// Reads and validates `FInternetAddrBSD::Addr` (`+0x08`) from a live UE4 `FInternetAddr*`.
///
/// # Safety
/// `addr_bsd` must be null or point to a live `FInternetAddrBSD` with `128` readable bytes at `+0x08`.
unsafe fn read_internet_addr_bsd(
    addr_bsd: *const c_void,
) -> Option<([u8; SOCKADDR_STORAGE_SIZE], i32)> {
    if addr_bsd.is_null() {
        return None;
    }
    let mut raw: [u8; SOCKADDR_STORAGE_SIZE] = [0u8; SOCKADDR_STORAGE_SIZE];
    // SAFETY: `FInternetAddrBSD::Addr` (`SOCKADDR_STORAGE`, 128 B) is verified at `+0x08`.
    unsafe {
        std::ptr::copy_nonoverlapping(
            addr_bsd
                .byte_add(OFFSET_FINTERNET_ADDR_BSD_STORAGE)
                .cast::<u8>(),
            raw.as_mut_ptr(),
            SOCKADDR_STORAGE_SIZE,
        );
    }
    parse_sockaddr_storage(&raw)
}

/// Captures the Dedicated Server game UDP endpoint on `FSocketBSD::SendTo` when `socket_ptr`
/// matches `UIpNetDriver::GetSocket()`.
///
/// # Safety
/// `dest_addr_bsd` must be null or point to a valid `FInternetAddrBSD`.
pub unsafe fn on_socket_send_to_observed(
    socket_ptr: usize,
    raw_socket: usize,
    dest_addr_bsd: *const c_void,
) {
    if socket_ptr == 0 || raw_socket == 0 || raw_socket == INVALID_RAW_SOCKET {
        return;
    }
    let expected_sock: usize = MATCH_GAME_SOCKET_PTR.load(Ordering::Acquire);
    if expected_sock == 0 || socket_ptr != expected_sock {
        return;
    }
    // Lock-free steady-state fast path once this game socket is already cached.
    if CACHED_ENDPOINT_SOCKET_PTR.load(Ordering::Acquire) == socket_ptr {
        return;
    }
    // SAFETY: `dest_addr_bsd` is passed directly from UE4's `FSocketBSD::SendTo`.
    let Some((remote_addr, remote_addr_len)): Option<([u8; SOCKADDR_STORAGE_SIZE], i32)> =
        (unsafe { read_internet_addr_bsd(dest_addr_bsd) })
    else {
        return;
    };
    store_game_endpoint(GameSocketEndpoint {
        socket_ptr,
        raw_socket,
        remote_addr,
        remote_addr_len,
    });
}

/// Clears any cached UDP endpoint referencing `socket_ptr` BEFORE `FSocketBSD::Close` closes
/// the underlying Win32 `SOCKET` handle.
pub fn on_socket_close_observed(socket_ptr: usize) {
    if socket_ptr == 0 {
        return;
    }
    if CACHED_ENDPOINT_SOCKET_PTR.load(Ordering::Acquire) != socket_ptr {
        return;
    }
    if let Ok(mut guard) = GAME_ENDPOINT.lock() {
        if let Some(ep) = *guard {
            if ep.socket_ptr == socket_ptr {
                *guard = None;
                CACHED_ENDPOINT_SOCKET_PTR.store(0, Ordering::Release);
            }
        }
    }
}

fn store_game_endpoint(endpoint: GameSocketEndpoint) {
    if let Ok(mut guard) = GAME_ENDPOINT.lock() {
        let changed: bool = *guard != Some(endpoint);
        *guard = Some(endpoint);
        CACHED_ENDPOINT_SOCKET_PTR.store(endpoint.socket_ptr, Ordering::Release);
        if changed {
            log_line(&format!(
                "captured game UDP socket endpoint (fsocket=0x{:X}, raw_sock=0x{:X}, addr_len={})",
                endpoint.socket_ptr, endpoint.raw_socket, endpoint.remote_addr_len
            ));
        }
    }
}

fn clear_game_endpoint() {
    if let Ok(mut guard) = GAME_ENDPOINT.lock() {
        *guard = None;
        CACHED_ENDPOINT_SOCKET_PTR.store(0, Ordering::Release);
    }
}

/// Sends a raw `DHV2` datagram directly over the active game UDP socket via `WinSock` `sendto`
/// while holding the short `GAME_ENDPOINT` lock (eliminating any `Close`/`sendto` handle race).
#[cfg(windows)]
pub fn send_raw_dhv2_packet(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.len() > MAX_PACKET_SIZE {
        return false;
    }
    let Ok(guard): Result<std::sync::MutexGuard<'_, Option<GameSocketEndpoint>>, _> =
        GAME_ENDPOINT.lock()
    else {
        return false;
    };
    let Some(ep): Option<GameSocketEndpoint> = *guard else {
        return false;
    };
    if ep.raw_socket == 0 || ep.raw_socket == INVALID_RAW_SOCKET {
        return false;
    }
    let sock: SOCKET = SOCKET(ep.raw_socket);
    let mut aligned_storage: SOCKADDR_STORAGE = SOCKADDR_STORAGE::default();
    // SAFETY: `SOCKADDR_STORAGE` is 128 bytes (`SOCKADDR_STORAGE_SIZE`) and 8-byte aligned.
    unsafe {
        std::ptr::copy_nonoverlapping(
            ep.remote_addr.as_ptr(),
            (&raw mut aligned_storage).cast::<u8>(),
            SOCKADDR_STORAGE_SIZE.min(std::mem::size_of::<SOCKADDR_STORAGE>()),
        );
    }
    let addr_ptr: *const SOCKADDR = (&raw const aligned_storage).cast::<SOCKADDR>();
    // SAFETY: `sock` is guarded against concurrent `FSocketBSD::Close` by `GAME_ENDPOINT`, and
    // `addr_ptr` points to a properly aligned, validated `SOCKADDR_IN` (`16 B`) or `SOCKADDR_IN6` (`28 B`).
    let sent: i32 = unsafe { sendto(sock, bytes, 0, addr_ptr, ep.remote_addr_len) };
    sent >= 0
}

/// Non-Windows stub returning `false`.
#[cfg(not(windows))]
pub fn send_raw_dhv2_packet(_bytes: &[u8]) -> bool {
    false
}

/// Processes a single verified `DHV2` datagram intercepted inside `FSocketBSD::RecvFrom`.
///
/// Safe to invoke from UE4's `FReceiveThreadRunnable` receive thread or game thread (touches
/// zero UE4 `UObjects`).
///
/// # Safety
/// `source_addr_bsd` must be null or point to a valid `FInternetAddrBSD`.
pub unsafe fn on_dhv2_datagram_received(
    socket_ptr: usize,
    raw_socket: usize,
    source_addr_bsd: *const c_void,
    packet: &[u8],
) {
    let Ok(msg): Result<Dhv2Message, dh_voice_proto::ProtoError> = decode_message(packet) else {
        return;
    };

    // Any valid server-sent `DHV2` control/voice datagram confirms the exact game UDP endpoint.
    if socket_ptr != 0
        && raw_socket != 0
        && raw_socket != INVALID_RAW_SOCKET
        && CACHED_ENDPOINT_SOCKET_PTR.load(Ordering::Acquire) != socket_ptr
    {
        // SAFETY: `source_addr_bsd` was just populated by `FSocketBSD::RecvFrom`.
        let addr_opt: Option<([u8; SOCKADDR_STORAGE_SIZE], i32)> =
            unsafe { read_internet_addr_bsd(source_addr_bsd) };
        if let Some((remote_addr, remote_addr_len)) = addr_opt {
            store_game_endpoint(GameSocketEndpoint {
                socket_ptr,
                raw_socket,
                remote_addr,
                remote_addr_len,
            });
        }
    }

    let now: Instant = Instant::now();
    match msg {
        Dhv2Message::HelloAck { peer_slot } => {
            ASSIGNED_PEER_SLOT.store(peer_slot, Ordering::Release);
            // Also refresh endpoint in case NAT port rebinding occurred.
            if socket_ptr != 0 && raw_socket != 0 && raw_socket != INVALID_RAW_SOCKET {
                // SAFETY: `source_addr_bsd` was populated by `FSocketBSD::RecvFrom`.
                let addr_opt: Option<([u8; SOCKADDR_STORAGE_SIZE], i32)> =
                    unsafe { read_internet_addr_bsd(source_addr_bsd) };
                if let Some((remote_addr, remote_addr_len)) = addr_opt {
                    store_game_endpoint(GameSocketEndpoint {
                        socket_ptr,
                        raw_socket,
                        remote_addr,
                        remote_addr_len,
                    });
                }
            }
        }
        Dhv2Message::PeerMap { entries } => {
            if let Ok(mut rx) = rx_state().lock() {
                rx.apply_peer_map(&entries);
            }
        }
        Dhv2Message::VoiceFrame {
            peer_slot,
            flags,
            seq,
            timestamp: _,
            payload,
        } => {
            let my_slot: u16 = ASSIGNED_PEER_SLOT.load(Ordering::Acquire);
            if my_slot != 0 && peer_slot == my_slot {
                return;
            }
            if let Ok(mut rx) = rx_state().lock() {
                rx.push_voice_frame(peer_slot, flags, seq, payload, now);
            }
        }
        Dhv2Message::Hello { .. } | Dhv2Message::Bye { .. } => {}
    }
}

/// Starts the Pure-Rust Dedicated Server voice session (Opus + WASAPI capture/loopback + DSP TX
/// + RX jitter table). Called once on the UE4 Game Thread when entering a Dedicated Server match.
pub fn start_match_session(opus_api: OpusApiTable) {
    ASSIGNED_PEER_SLOT.store(0, Ordering::Release);

    if let Ok(mut rx) = rx_state().lock() {
        rx.reset_for_new_match(Some(opus_api));
    }

    let encoder_res: Result<OpusEncoder, crate::structs::opus::OpusError> =
        OpusEncoder::new_voice_48k_mono(opus_api);
    let wasapi_session: Option<WasapiSession> = match encoder_res {
        Ok(encoder) => {
            let factory: CaptureFrameFactory = Box::new(move || -> CaptureFrameHandler {
                let mut tx: TxCapturePipeline = TxCapturePipeline::new(encoder);
                Box::new(
                    move |cap_frame: &[f32; DSP_FRAME_SAMPLES_10MS],
                          render_opt: Option<&[f32; DSP_FRAME_SAMPLES_10MS]>| {
                        tx.on_capture_10ms(cap_frame, render_opt);
                    },
                )
            });
            WasapiSession::start(factory)
        }
        Err(err) => {
            log_line(&format!(
                "failed to initialize in-process libopus encoder: {err:?}"
            ));
            None
        }
    };

    if let Ok(mut session) = session_state().lock() {
        if let Some(mut prev) = session.wasapi.take() {
            prev.stop();
        }
        session.wasapi = wasapi_session;
        session.local_uid = None;
        session.last_hello_sent = None;
        session.logged_ack_slot = 0;
    }
}

/// Stops the Dedicated Server voice session cleanly: transmits `DHV2::Bye`, stops & joins
/// WASAPI threads (releasing mic handles for Lobby EOS), and clears RX/endpoint state.
pub fn stop_match_session() {
    let slot: u16 = ASSIGNED_PEER_SLOT.swap(0, Ordering::AcqRel);
    let mut bye_buf: [u8; MAX_PACKET_SIZE] = [0u8; MAX_PACKET_SIZE];
    if let Ok(len) = encode_bye_into(&mut bye_buf, slot) {
        let _sent: bool = send_raw_dhv2_packet(&bye_buf[..len]);
    }

    LOCAL_WANTS_TO_TALK.store(false, Ordering::Release);
    MATCH_GAME_SOCKET_PTR.store(0, Ordering::Release);

    if let Ok(mut session) = session_state().lock() {
        if let Some(mut wasapi) = session.wasapi.take() {
            wasapi.stop();
        }
        session.local_uid = None;
        session.last_hello_sent = None;
        session.logged_ack_slot = 0;
    }

    if let Ok(mut rx) = rx_state().lock() {
        rx.reset_for_new_match(None);
    }

    clear_game_endpoint();
}

/// Drives initial `DHV2::Hello` registration (`1 s` retry until `HelloAck`) and `3 s` steady-state
/// keepalives on the UE4 Game Thread.
pub fn tick_hello_keepalive(discovered_local_uid: Option<&str>, now: Instant) {
    let Ok(mut session): Result<std::sync::MutexGuard<'_, ClientVoiceSession>, _> =
        session_state().lock()
    else {
        return;
    };

    let current_slot: u16 = ASSIGNED_PEER_SLOT.load(Ordering::Acquire);
    if current_slot != 0 && session.logged_ack_slot != current_slot {
        session.logged_ack_slot = current_slot;
        log_line(&format!(
            "DHV2 HelloAck received: assigned peer_slot={current_slot}"
        ));
    }

    if let Some(uid) = discovered_local_uid {
        if !uid.is_empty() && session.local_uid.as_deref() != Some(uid) {
            session.local_uid = Some(uid.to_owned());
            session.last_hello_sent = None;
        }
    }

    let Some(uid_ref): Option<&str> = session.local_uid.as_deref() else {
        return;
    };
    if CACHED_ENDPOINT_SOCKET_PTR.load(Ordering::Acquire) == 0 {
        return;
    }

    let interval_ms: u64 = if current_slot == 0 {
        HELLO_RETRY_INTERVAL_MS
    } else {
        HELLO_KEEPALIVE_INTERVAL_MS
    };
    let should_send: bool = match session.last_hello_sent {
        None => true,
        Some(prev) => now.saturating_duration_since(prev) >= Duration::from_millis(interval_ms),
    };
    if !should_send {
        return;
    }

    let mut hello_buf: [u8; MAX_PACKET_SIZE] = [0u8; MAX_PACKET_SIZE];
    if let Ok(wire_len) = encode_hello_into(&mut hello_buf, uid_ref) {
        if send_raw_dhv2_packet(&hello_buf[..wire_len]) {
            session.last_hello_sent = Some(now);
        }
    }
}

/// Advances the global RX playout cadence (`<= 100 ms` cap) and collects decoded `20 ms` PCM
/// frames into `out` so the UE4 Game Thread can deliver them without holding any engine mutex.
pub fn drain_ready_playout_frames(now: Instant, out: &mut Vec<DecodedPlayoutFrame>) {
    out.clear();
    if let Ok(mut rx) = rx_state().lock() {
        rx.drain_playout_frames(now, out);
    }
}

impl TxCapturePipeline {
    /// Creates a new capture TX pipeline wrapping a fresh pure-Rust [`DspPipeline`] and
    /// configured in-process [`OpusEncoder`].
    #[must_use]
    pub fn new(encoder: OpusEncoder) -> Self {
        Self {
            dsp: DspPipeline::new(),
            encoder,
            pcm_20ms: [0i16; OPUS_FRAME_SAMPLES_20MS],
            has_first_half: false,
            first_half_speech: false,
            in_talkspurt: false,
            tx_seq: 0,
            tx_timestamp: 0,
            opus_scratch: [0u8; MAX_OPUS_PAYLOAD],
            wire_scratch: [0u8; MAX_PACKET_SIZE],
        }
    }

    /// Handles one `10 ms` (`480`-sample @ `48 kHz`) captured microphone frame and optional
    /// loopback render reference frame on the WASAPI capture thread.
    pub fn on_capture_10ms(
        &mut self,
        capture_mono_f32: &[f32; DSP_FRAME_SAMPLES_10MS],
        render_mono_f32: Option<&[f32; DSP_FRAME_SAMPLES_10MS]>,
    ) {
        let wants_talk: bool = LOCAL_WANTS_TO_TALK.load(Ordering::Acquire);
        let peer_slot: u16 = ASSIGNED_PEER_SLOT.load(Ordering::Acquire);

        if !wants_talk {
            if self.in_talkspurt {
                if peer_slot != 0 {
                    self.emit_end_of_talkspurt(peer_slot);
                }
                self.in_talkspurt = false;
            }
            self.has_first_half = false;
            self.first_half_speech = false;
            self.dsp.warmup_aec_10ms(capture_mono_f32, render_mono_f32);
            return;
        }

        let dsp_res: crate::structs::dsp::DspFrameResult = self
            .dsp
            .process_capture_10ms(capture_mono_f32, render_mono_f32);

        if !self.has_first_half {
            self.pcm_20ms[..DSP_FRAME_SAMPLES_10MS].copy_from_slice(&dsp_res.pcm_i16);
            self.first_half_speech = dsp_res.speech_active;
            self.has_first_half = true;
            return;
        }

        self.pcm_20ms[DSP_FRAME_SAMPLES_10MS..OPUS_FRAME_SAMPLES_20MS]
            .copy_from_slice(&dsp_res.pcm_i16);
        let speech_active: bool = self.first_half_speech || dsp_res.speech_active;
        self.has_first_half = false;
        self.first_half_speech = false;

        if peer_slot == 0 {
            return;
        }

        if speech_active {
            match self
                .encoder
                .encode_20ms(&self.pcm_20ms, &mut self.opus_scratch)
            {
                Ok(enc_len) if enc_len > OPUS_DTX_SILENCE_MAX_BYTES => {
                    let payload: &[u8] = &self.opus_scratch[..enc_len];
                    if let Ok(wire_len) = encode_voice_frame_into(
                        &mut self.wire_scratch,
                        peer_slot,
                        FLAG_HAS_FEC,
                        self.tx_seq,
                        self.tx_timestamp,
                        payload,
                    ) {
                        let _sent: bool = send_raw_dhv2_packet(&self.wire_scratch[..wire_len]);
                        self.tx_seq = self.tx_seq.wrapping_add(1);
                        self.tx_timestamp = self
                            .tx_timestamp
                            .wrapping_add(u32::try_from(OPUS_FRAME_SAMPLES_20MS).unwrap_or(960));
                        self.in_talkspurt = true;
                    }
                }
                Ok(_) => {
                    // Opus DTX silence/comfort packet (`<= 2 B`): suppress TX cleanly.
                    if self.in_talkspurt {
                        self.emit_end_of_talkspurt(peer_slot);
                        self.in_talkspurt = false;
                    }
                }
                Err(_) => {}
            }
        } else if self.in_talkspurt {
            self.emit_end_of_talkspurt(peer_slot);
            self.in_talkspurt = false;
        }
    }

    fn emit_end_of_talkspurt(&mut self, peer_slot: u16) {
        if let Ok(wire_len) = encode_voice_frame_into(
            &mut self.wire_scratch,
            peer_slot,
            FLAG_END_OF_TALKSPURT,
            self.tx_seq,
            self.tx_timestamp,
            &[],
        ) {
            let _sent: bool = send_raw_dhv2_packet(&self.wire_scratch[..wire_len]);
            self.tx_seq = self.tx_seq.wrapping_add(1);
            self.tx_timestamp = self
                .tx_timestamp
                .wrapping_add(u32::try_from(OPUS_FRAME_SAMPLES_20MS).unwrap_or(960));
        }
    }
}

impl VoiceRxState {
    /// Creates an empty receive table with an optional resolved `OpusApiTable`.
    #[must_use]
    pub fn new(opus_api: Option<OpusApiTable>) -> Self {
        Self {
            epoch: Instant::now(),
            opus_api,
            peers: HashMap::new(),
            last_playout_tick: None,
            playout_accum_ms: 0,
        }
    }

    /// Resets all remote peers, jitter buffers, decoders, and playout timing for a fresh match.
    pub fn reset_for_new_match(&mut self, opus_api: Option<OpusApiTable>) {
        self.epoch = Instant::now();
        self.opus_api = opus_api;
        self.peers.clear();
        self.last_playout_tick = None;
        self.playout_accum_ms = 0;
    }

    /// Number of currently tracked remote peer slots.
    #[must_use]
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Returns the mapped UTF-8 UID for `peer_slot`, if known.
    #[must_use]
    pub fn peer_uid(&self, peer_slot: u16) -> Option<&str> {
        self.peers
            .get(&peer_slot)
            .filter(|peer: &&RemotePeerVoice| !peer.uid_utf8.is_empty())
            .map(|peer: &RemotePeerVoice| peer.uid_utf8.as_str())
    }

    /// Applies an authoritative server `PeerMap` snapshot, updating UIDs, pruning departed slots,
    /// and resetting jitter/decoder state whenever a slot is reassigned to a different player.
    pub fn apply_peer_map(&mut self, entries: &[PeerMapEntry]) {
        let mut active_slots: HashSet<u16> = HashSet::with_capacity(entries.len());
        for entry in entries {
            if !(MIN_PEER_SLOT..=MAX_PEER_SLOT).contains(&entry.peer_slot) {
                continue;
            }
            let _inserted: bool = active_slots.insert(entry.peer_slot);

            if let Some(existing) = self.peers.get_mut(&entry.peer_slot) {
                if existing.uid_utf8 != entry.uid {
                    let was_mapped: bool = !existing.uid_utf8.is_empty();
                    existing.uid_utf8.clone_from(&entry.uid);
                    existing.uid_utf16_nul = encode_uid_utf16_nul(&entry.uid);
                    if was_mapped {
                        existing.jitter.reset();
                        if let Some(decoder) = existing.decoder.as_mut() {
                            let _reset: Result<(), crate::structs::opus::OpusError> =
                                decoder.reset();
                        }
                    }
                }
            } else if self.peers.len() < JITTER_MAX_SPEAKERS {
                let _prev: Option<RemotePeerVoice> = self.peers.insert(
                    entry.peer_slot,
                    RemotePeerVoice {
                        uid_utf8: entry.uid.clone(),
                        uid_utf16_nul: encode_uid_utf16_nul(&entry.uid),
                        jitter: SpeakerJitterBuffer::new(),
                        decoder: None,
                    },
                );
            }
        }

        self.peers
            .retain(|slot: &u16, _peer: &mut RemotePeerVoice| active_slots.contains(slot));
    }

    /// Inserts an arriving `DHV2::VoiceFrame` into `peer_slot`'s adaptive jitter buffer.
    pub fn push_voice_frame(
        &mut self,
        peer_slot: u16,
        flags: u8,
        seq: u16,
        payload: Vec<u8>,
        now: Instant,
    ) {
        if !(MIN_PEER_SLOT..=MAX_PEER_SLOT).contains(&peer_slot) {
            return;
        }
        let arrival_ms: u64 = u64::try_from(now.saturating_duration_since(self.epoch).as_millis())
            .unwrap_or(u64::MAX);

        if let Some(peer) = self.peers.get_mut(&peer_slot) {
            peer.jitter.push_packet(seq, flags, payload, arrival_ms);
            return;
        }
        if self.peers.len() >= JITTER_MAX_SPEAKERS {
            return;
        }
        let mut jitter: SpeakerJitterBuffer = SpeakerJitterBuffer::new();
        jitter.push_packet(seq, flags, payload, arrival_ms);
        let _prev: Option<RemotePeerVoice> = self.peers.insert(
            peer_slot,
            RemotePeerVoice {
                uid_utf8: String::new(),
                uid_utf16_nul: Vec::new(),
                jitter,
                decoder: None,
            },
        );
    }

    /// Steps the paced `20 ms` playout clock (`<= 100 ms` burst cap) and decodes ready remote
    /// voice frames (normal, In-Band FEC, and bounded `<= 3`-frame faded PLC) into `out`.
    pub fn drain_playout_frames(&mut self, now: Instant, out: &mut Vec<DecodedPlayoutFrame>) {
        let elapsed_ms: u64 = match self.last_playout_tick {
            Some(prev) => {
                u64::try_from(now.saturating_duration_since(prev).as_millis()).unwrap_or(u64::MAX)
            }
            None => 0,
        };
        self.last_playout_tick = Some(now);

        let steps: usize = advance_playout_accumulator(&mut self.playout_accum_ms, elapsed_ms);
        if steps == 0 {
            return;
        }

        let Some(api): Option<OpusApiTable> = self.opus_api else {
            return;
        };

        for peer in self.peers.values_mut() {
            if peer.uid_utf16_nul.len() < 2 {
                continue;
            }
            for _step in 0..steps {
                let action: PlayoutAction = peer.jitter.pop_playout_action();
                if matches!(action, PlayoutAction::Silence) {
                    break;
                }

                if peer.decoder.is_none() {
                    if let Ok(dec) = OpusDecoder::new_voice_48k_mono(api) {
                        peer.decoder = Some(dec);
                    } else {
                        break;
                    }
                }
                let Some(decoder): Option<&mut OpusDecoder> = peer.decoder.as_mut() else {
                    break;
                };

                let mut pcm: [i16; OPUS_FRAME_SAMPLES_20MS] = [0i16; OPUS_FRAME_SAMPLES_20MS];
                let decoded_ok: bool = match action {
                    PlayoutAction::DecodeFrame { payload } => {
                        matches!(decoder.decode_20ms(Some(&payload), &mut pcm, false), Ok(n) if n > 0)
                    }
                    PlayoutAction::DecodeFec { next_payload } => {
                        matches!(decoder.decode_20ms(Some(&next_payload), &mut pcm, true), Ok(n) if n > 0)
                    }
                    PlayoutAction::Plc { loss_streak } => {
                        if matches!(decoder.decode_20ms(None, &mut pcm, false), Ok(n) if n > 0) {
                            apply_plc_fade_in_place(&mut pcm, loss_streak);
                            true
                        } else {
                            false
                        }
                    }
                    PlayoutAction::Silence => false,
                };

                if decoded_ok {
                    out.push(DecodedPlayoutFrame {
                        uid_utf16_nul: peer.uid_utf16_nul.clone(),
                        pcm_i16: pcm,
                    });
                }
            }
        }
    }
}
