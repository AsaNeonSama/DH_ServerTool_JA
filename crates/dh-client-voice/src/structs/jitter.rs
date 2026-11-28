use std::collections::VecDeque;

/// Single received Opus voice frame queued inside a per-speaker adaptive jitter buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferedVoicePacket {
    /// Wrapping 16-bit sequence number from `DHV2` header.
    pub seq: u16,
    /// `DHV2` flags byte (`bit0 = END_OF_TALKSPURT`, `bit1 = HAS_FEC`).
    pub flags: u8,
    /// Raw Opus encoded bytes (`<= 512` bytes).
    pub payload: Vec<u8>,
}

/// Playout instruction emitted by [`SpeakerJitterBuffer::pop_playout_action`] every `20 ms`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayoutAction {
    /// Decode the next in-order Opus frame normally (`decode_fec = false`).
    DecodeFrame { payload: Vec<u8> },
    /// Recover a single lost packet via In-Band FEC from the immediately following packet (`decode_fec = true`).
    DecodeFec { next_payload: Vec<u8> },
    /// Synthesize Packet Loss Concealment (`packet = None`) with fade-out index `1..=3`.
    Plc { loss_streak: u32 },
    /// No active audio for this speaker right now (stay silent).
    Silence,
}

/// Diagnostic counters for a speaker's adaptive jitter buffer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JitterStats {
    pub received_packets: u64,
    pub played_frames: u64,
    pub fec_recoveries: u64,
    pub plc_frames: u64,
    pub late_or_duplicate_drops: u64,
    pub overflow_drops: u64,
}

/// Per-speaker adaptive jitter buffer (`40 ms..=120 ms`) with sequence-wrap handling,
/// 1-packet Opus In-Band FEC lookahead, and bounded `<= 3`-frame PLC fade-out.
#[derive(Debug, Clone)]
pub struct SpeakerJitterBuffer {
    pub(crate) packets: VecDeque<BufferedVoicePacket>,
    pub(crate) in_talkspurt: bool,
    pub(crate) next_seq: u16,
    pub(crate) target_frames: usize,
    pub(crate) consecutive_loss: u32,
    pub(crate) stable_hits: u32,
    pub(crate) jitter_ms_ewma: f32,
    pub(crate) last_arrival_ms: Option<u64>,
    pub(crate) last_arrival_seq: Option<u16>,
    pub(crate) stats: JitterStats,
}
