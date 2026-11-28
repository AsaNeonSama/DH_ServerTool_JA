use std::collections::HashMap;
use std::time::Instant;

use dh_voice_proto::{MAX_OPUS_PAYLOAD, MAX_PACKET_SIZE};

use crate::consts::dsp::OPUS_FRAME_SAMPLES_20MS;
use crate::consts::engine::SOCKADDR_STORAGE_SIZE;
use crate::structs::dsp::DspPipeline;
use crate::structs::jitter::SpeakerJitterBuffer;
use crate::structs::opus::{OpusApiTable, OpusDecoder, OpusEncoder};
use crate::structs::wasapi::WasapiSession;

/// Captured active game UDP socket and server `SOCKADDR_STORAGE` endpoint used for `DHV2` TX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameSocketEndpoint {
    /// Live UE4 `FSocketBSD*` pointer matching `UIpNetDriver::GetSocket()`.
    pub socket_ptr: usize,
    /// Underlying Win32 `SOCKET` handle read from `FSocketBSD + 0x28`.
    pub raw_socket: usize,
    /// Verbatim `SOCKADDR_STORAGE` bytes copied from `FInternetAddrBSD + 0x08`.
    pub remote_addr: [u8; SOCKADDR_STORAGE_SIZE],
    /// Effective byte length (`16` for `AF_INET`, `28` for `AF_INET6`).
    pub remote_addr_len: i32,
}

/// Per-speaker receive pipeline holding mapped UID, adaptive jitter buffer, and Opus decoder.
pub struct RemotePeerVoice {
    /// UTF-8 player UID assigned by the server's `PeerMap` (empty until `PeerMap` arrives).
    pub uid_utf8: String,
    /// Pre-encoded NUL-terminated UTF-16LE UID buffer passed as read-only UE4 `FString`.
    pub uid_utf16_nul: Vec<u16>,
    /// Adaptive `40..=120 ms` jitter buffer with 1-packet FEC lookahead and bounded PLC.
    pub jitter: SpeakerJitterBuffer,
    /// Dedicated per-speaker `48 kHz` mono Opus decoder (created lazily once `OpusApiTable` is live).
    pub decoder: Option<OpusDecoder>,
}

/// Decoded `20 ms` (`960`-sample @ `48 kHz` mono) remote speaker audio ready to deliver on the
/// UE4 Game Thread into `UDH_OnlineVoice::BeforeRecvAudioRendered`.
#[derive(Debug, Clone)]
pub struct DecodedPlayoutFrame {
    /// NUL-terminated UTF-16LE sender UID (`uid_utf16_nul.len() >= 2`).
    pub uid_utf16_nul: Vec<u16>,
    /// Decoded `48 kHz` mono signed 16-bit PCM samples (`960` samples = `20 ms`).
    pub pcm_i16: [i16; OPUS_FRAME_SAMPLES_20MS],
}

/// Thread-safe receive table mapping server-assigned `peer_slot` (`1..=64`) to per-speaker RX state.
pub struct VoiceRxState {
    /// Epoch timestamp used to produce monotonic arrival milliseconds for jitter estimation.
    pub epoch: Instant,
    /// Resolved in-process `libopus` API table for creating per-speaker decoders.
    pub opus_api: Option<OpusApiTable>,
    /// Active remote peers indexed by authoritative `peer_slot` (`1..=64`).
    pub peers: HashMap<u16, RemotePeerVoice>,
    /// Last monotonic timestamp when `drain_playout_frames` ticked.
    pub last_playout_tick: Option<Instant>,
    /// Residual playout cadence accumulator in milliseconds (strictly capped `<= 100 ms`).
    pub playout_accum_ms: u64,
}

/// Transmit pipeline owned exclusively by the WASAPI capture worker thread.
pub struct TxCapturePipeline {
    /// Full `48 kHz` mono pure-Rust DSP chain (`HPF -> AEC3 -> RNNoise -> VAD -> AGC/Limiter`).
    pub dsp: DspPipeline,
    /// In-process `libopus` `48 kHz` mono VOIP encoder (`32 kbps`, In-Band FEC, DTX).
    pub encoder: OpusEncoder,
    /// Assembly buffer combining two `10 ms` (`480`-sample) DSP frames into one `20 ms` (`960`-sample) Opus frame.
    pub pcm_20ms: [i16; OPUS_FRAME_SAMPLES_20MS],
    /// Whether the first `10 ms` half (`0..480`) is currently buffered.
    pub has_first_half: bool,
    /// Whether the first `10 ms` half had `speech_active == true`.
    pub first_half_speech: bool,
    /// Whether an active talkspurt is currently being transmitted (controls `FLAG_END_OF_TALKSPURT`).
    pub in_talkspurt: bool,
    /// Monotonic wrapping 16-bit sequence number for outgoing `DHV2::VoiceFrame` packets.
    pub tx_seq: u16,
    /// Monotonic `48 kHz` sample timestamp (`+960` per `20 ms` frame).
    pub tx_timestamp: u32,
    /// Smoothed local microphone energy (`0.0..=1.0`) published lock-free for HUD volume meters.
    pub smoothed_energy: f32,
    /// Reusable Opus output scratch buffer (zero per-frame heap allocation).
    pub opus_scratch: [u8; MAX_OPUS_PAYLOAD],
    /// Reusable `DHV2` wire packet scratch buffer (zero per-frame heap allocation).
    pub wire_scratch: [u8; MAX_PACKET_SIZE],
}

/// Active match session state owned by the client voice engine while on a Dedicated Server.
pub struct ClientVoiceSession {
    /// Running WASAPI microphone capture + loopback render-reference session.
    pub wasapi: Option<WasapiSession>,
    /// Local player UTF-8 UID string sent in `DHV2::Hello` registration/keepalive packets.
    pub local_uid: Option<String>,
    /// Timestamp of the most recently transmitted `DHV2::Hello` packet.
    pub last_hello_sent: Option<Instant>,
    /// Whether we have already logged the first `HelloAck` assignment for this match.
    pub logged_ack_slot: u16,
}
