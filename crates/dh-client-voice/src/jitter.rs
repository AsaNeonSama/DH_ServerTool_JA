use std::collections::VecDeque;

use dh_voice_proto::{FLAG_END_OF_TALKSPURT, FLAG_HAS_FEC};

use crate::consts::jitter::{
    JITTER_FRAME_DURATION_MS, JITTER_INITIAL_TARGET_FRAMES, JITTER_MAX_BUFFERED_FRAMES,
    JITTER_MAX_PLAYOUT_FRAMES_PER_TICK, JITTER_MAX_REORDER_WINDOW, JITTER_MAX_TARGET_FRAMES,
    JITTER_MAX_TICK_DELTA_MS, JITTER_MIN_TARGET_FRAMES, JITTER_PLC_FADE_GAINS,
    JITTER_SHRINK_STABLE_FRAMES,
};
pub use crate::structs::jitter::{
    BufferedVoicePacket, JitterStats, PlayoutAction, SpeakerJitterBuffer,
};

/// Signed wrapping 16-bit sequence delta (`a - b` in `-32768..=32767`).
#[must_use]
pub fn seq_diff(a: u16, b: u16) -> i16 {
    a.wrapping_sub(b).cast_signed()
}

/// Computes how many `20 ms` playout frames should be emitted this tick, enforcing the
/// `<= 100 ms` (`5`-frame) burst playout cap.
#[must_use]
pub fn advance_playout_accumulator(accumulator_ms: &mut u64, elapsed_ms: u64) -> usize {
    let clamped_elapsed: u64 = elapsed_ms.min(JITTER_MAX_TICK_DELTA_MS);
    *accumulator_ms = accumulator_ms
        .saturating_add(clamped_elapsed)
        .min(JITTER_MAX_TICK_DELTA_MS);

    let frame_ms: u64 = u64::from(JITTER_FRAME_DURATION_MS);
    let raw_frames: u64 = *accumulator_ms / frame_ms;
    let capped_frames: usize = usize::try_from(raw_frames)
        .unwrap_or(JITTER_MAX_PLAYOUT_FRAMES_PER_TICK)
        .min(JITTER_MAX_PLAYOUT_FRAMES_PER_TICK);
    let consumed_ms: u64 = u64::try_from(capped_frames)
        .unwrap_or(0)
        .saturating_mul(frame_ms);
    *accumulator_ms = accumulator_ms.saturating_sub(consumed_ms);
    capped_frames
}

/// Returns the smooth attenuation gain for a PLC loss streak (`1..=3`).
#[must_use]
pub fn plc_fade_gain(loss_streak: u32) -> f32 {
    let idx: usize = usize::try_from(loss_streak.saturating_sub(1))
        .unwrap_or(JITTER_PLC_FADE_GAINS.len() - 1)
        .min(JITTER_PLC_FADE_GAINS.len() - 1);
    JITTER_PLC_FADE_GAINS[idx]
}

impl Default for SpeakerJitterBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeakerJitterBuffer {
    /// Creates an empty per-speaker adaptive jitter buffer starting at `60 ms` (`3` frames).
    #[must_use]
    pub fn new() -> Self {
        Self {
            packets: VecDeque::with_capacity(JITTER_MAX_BUFFERED_FRAMES),
            in_talkspurt: false,
            next_seq: 0,
            target_frames: JITTER_INITIAL_TARGET_FRAMES,
            consecutive_loss: 0,
            stable_hits: 0,
            jitter_ms_ewma: 0.0,
            last_arrival_ms: None,
            last_arrival_seq: None,
            stats: JitterStats::default(),
        }
    }

    /// Clears all buffered packets and resets talkspurt playback state (retaining adaptive target).
    pub fn reset(&mut self) {
        self.packets.clear();
        self.in_talkspurt = false;
        self.consecutive_loss = 0;
        self.stable_hits = 0;
        self.last_arrival_ms = None;
        self.last_arrival_seq = None;
    }

    /// Current adaptive buffering target in `20 ms` frames (`2..=6` = `40 ms..=120 ms`).
    #[must_use]
    pub fn target_frames(&self) -> usize {
        self.target_frames
    }

    /// Number of voice packets currently waiting in the reorder queue.
    #[must_use]
    pub fn buffered_len(&self) -> usize {
        self.packets.len()
    }

    /// Returns whether this speaker is actively playing out a talkspurt.
    #[must_use]
    pub fn is_in_talkspurt(&self) -> bool {
        self.in_talkspurt
    }

    /// Returns cumulative diagnostic counters for this speaker's jitter buffer.
    #[must_use]
    pub fn stats(&self) -> JitterStats {
        self.stats
    }

    /// Pushes a newly arrived `DHV2` voice packet with arrival timestamp `arrival_ms` (monotonic ms).
    pub fn push_packet(&mut self, seq: u16, flags: u8, payload: Vec<u8>, arrival_ms: u64) {
        self.stats.received_packets = self.stats.received_packets.saturating_add(1);
        self.update_jitter_estimate(seq, arrival_ms);

        // If actively playing, reject late packets behind `next_seq` and handle huge discontinuities.
        if self.in_talkspurt {
            let delta: i16 = seq_diff(seq, self.next_seq);
            if delta < 0 {
                if delta > -JITTER_MAX_REORDER_WINDOW {
                    self.stats.late_or_duplicate_drops =
                        self.stats.late_or_duplicate_drops.saturating_add(1);
                    return;
                }
                // Large discontinuity (e.g. sender restarted): reset buffer to new sequence epoch.
                self.packets.clear();
                self.in_talkspurt = false;
                self.consecutive_loss = 0;
            } else if delta > JITTER_MAX_REORDER_WINDOW {
                self.packets.clear();
                self.in_talkspurt = false;
                self.consecutive_loss = 0;
            }
        }

        // Insert into `self.packets` in ascending wrapping sequence order, deduplicating exact seqs.
        let mut insert_idx: usize = self.packets.len();
        for (idx, existing) in self.packets.iter().enumerate() {
            let cmp: i16 = seq_diff(seq, existing.seq);
            if cmp == 0 {
                self.stats.late_or_duplicate_drops =
                    self.stats.late_or_duplicate_drops.saturating_add(1);
                return;
            }
            if cmp < 0 {
                insert_idx = idx;
                break;
            }
        }

        self.packets.insert(
            insert_idx,
            BufferedVoicePacket {
                seq,
                flags,
                payload,
            },
        );

        // Enforce hard queue cap so latency never balloons under severe bursts.
        while self.packets.len() > JITTER_MAX_BUFFERED_FRAMES {
            let _dropped: Option<BufferedVoicePacket> = self.packets.pop_front();
            self.stats.overflow_drops = self.stats.overflow_drops.saturating_add(1);
            if self.in_talkspurt {
                if let Some(front) = self.packets.front() {
                    if seq_diff(front.seq, self.next_seq) > 0 {
                        self.next_seq = front.seq;
                    }
                }
            }
        }
    }

    /// Pops the next `20 ms` playout action (`DecodeFrame`, `DecodeFec`, `Plc`, or `Silence`).
    pub fn pop_playout_action(&mut self) -> PlayoutAction {
        if !self.in_talkspurt {
            if self.packets.is_empty() {
                return PlayoutAction::Silence;
            }
            let has_end_marker: bool = self
                .packets
                .iter()
                .any(|pkt: &BufferedVoicePacket| (pkt.flags & FLAG_END_OF_TALKSPURT) != 0);
            if self.packets.len() < self.target_frames && !has_end_marker {
                return PlayoutAction::Silence;
            }
            // Start talkspurt from earliest buffered packet.
            if let Some(first) = self.packets.front() {
                self.next_seq = first.seq;
                self.in_talkspurt = true;
                self.consecutive_loss = 0;
            } else {
                return PlayoutAction::Silence;
            }
        }

        // Discard any stale packets that fell behind `next_seq`.
        while let Some(front) = self.packets.front() {
            if seq_diff(front.seq, self.next_seq) < 0 {
                let _stale: Option<BufferedVoicePacket> = self.packets.pop_front();
                self.stats.late_or_duplicate_drops =
                    self.stats.late_or_duplicate_drops.saturating_add(1);
            } else {
                break;
            }
        }

        // 1. Exact sequence match: normal decode.
        if let Some(front) = self.packets.front() {
            if front.seq == self.next_seq {
                if let Some(pkt) = self.packets.pop_front() {
                    self.next_seq = self.next_seq.wrapping_add(1);
                    self.consecutive_loss = 0;
                    self.stable_hits = self.stable_hits.saturating_add(1);
                    self.maybe_shrink_target();
                    self.stats.played_frames = self.stats.played_frames.saturating_add(1);

                    let ended: bool = (pkt.flags & FLAG_END_OF_TALKSPURT) != 0;
                    if ended && self.packets.is_empty() {
                        self.in_talkspurt = false;
                    }
                    if pkt.payload.is_empty() {
                        return PlayoutAction::Silence;
                    }
                    return PlayoutAction::DecodeFrame {
                        payload: pkt.payload,
                    };
                }
            }
        }

        // 2. Missing `next_seq`: check 1-packet Opus In-Band FEC lookahead on `next_seq + 1`.
        let next_plus_one: u16 = self.next_seq.wrapping_add(1);
        if self.consecutive_loss == 0 {
            if let Some(front) = self.packets.front() {
                if front.seq == next_plus_one
                    && (front.flags & FLAG_HAS_FEC) != 0
                    && !front.payload.is_empty()
                {
                    let fec_payload: Vec<u8> = front.payload.clone();
                    self.next_seq = next_plus_one;
                    self.consecutive_loss = 1;
                    self.stable_hits = 0;
                    self.stats.fec_recoveries = self.stats.fec_recoveries.saturating_add(1);
                    return PlayoutAction::DecodeFec {
                        next_payload: fec_payload,
                    };
                }
            }
        }

        // 3. Missing `next_seq` without FEC: synthesize bounded PLC (`<= 3` frames = `60 ms`).
        if self.consecutive_loss < crate::consts::jitter::JITTER_MAX_PLC_FRAMES {
            self.consecutive_loss = self.consecutive_loss.saturating_add(1);
            self.next_seq = self.next_seq.wrapping_add(1);
            self.stable_hits = 0;
            self.widen_target_on_underrun();
            self.stats.plc_frames = self.stats.plc_frames.saturating_add(1);
            return PlayoutAction::Plc {
                loss_streak: self.consecutive_loss,
            };
        }

        // 4. PLC budget exhausted (`>= 3` frames): resync to queued future packet or end talkspurt.
        if let Some(front) = self.packets.front() {
            self.next_seq = front.seq;
            self.consecutive_loss = 0;
            if let Some(pkt) = self.packets.pop_front() {
                self.next_seq = self.next_seq.wrapping_add(1);
                self.stats.played_frames = self.stats.played_frames.saturating_add(1);
                let ended: bool = (pkt.flags & FLAG_END_OF_TALKSPURT) != 0;
                if ended && self.packets.is_empty() {
                    self.in_talkspurt = false;
                }
                if pkt.payload.is_empty() {
                    return PlayoutAction::Silence;
                }
                return PlayoutAction::DecodeFrame {
                    payload: pkt.payload,
                };
            }
        }

        self.in_talkspurt = false;
        self.consecutive_loss = 0;
        PlayoutAction::Silence
    }

    fn update_jitter_estimate(&mut self, seq: u16, arrival_ms: u64) {
        if let (Some(prev_ms), Some(prev_seq)) = (self.last_arrival_ms, self.last_arrival_seq) {
            let d_seq: i16 = seq_diff(seq, prev_seq);
            if d_seq > 0 && d_seq <= 50 {
                let expected_dt: i64 = i64::from(d_seq) * i64::from(JITTER_FRAME_DURATION_MS);
                let actual_dt: i64 =
                    i64::try_from(arrival_ms.saturating_sub(prev_ms)).unwrap_or(expected_dt);
                let transit_err_ms: u32 =
                    u32::try_from((actual_dt - expected_dt).abs().min(500)).unwrap_or(500);
                let err_f32: f32 = f32::from(u16::try_from(transit_err_ms).unwrap_or(500));
                self.jitter_ms_ewma += (err_f32 - self.jitter_ms_ewma) / 16.0;

                // Map EWMA jitter cleanly to target depth `2..=6` frames (`40..=120 ms`).
                let desired_from_jitter: usize = if self.jitter_ms_ewma >= 55.0 {
                    JITTER_MAX_TARGET_FRAMES
                } else if self.jitter_ms_ewma >= 38.0 {
                    5
                } else if self.jitter_ms_ewma >= 24.0 {
                    4
                } else if self.jitter_ms_ewma >= 12.0 {
                    3
                } else {
                    JITTER_MIN_TARGET_FRAMES
                };
                if desired_from_jitter > self.target_frames {
                    self.target_frames = desired_from_jitter;
                    self.stable_hits = 0;
                }
            }
        }
        self.last_arrival_ms = Some(arrival_ms);
        self.last_arrival_seq = Some(seq);
    }

    fn widen_target_on_underrun(&mut self) {
        if self.target_frames < JITTER_MAX_TARGET_FRAMES {
            self.target_frames += 1;
        }
    }

    fn maybe_shrink_target(&mut self) {
        if self.stable_hits >= JITTER_SHRINK_STABLE_FRAMES
            && self.target_frames > JITTER_MIN_TARGET_FRAMES
            && self.jitter_ms_ewma < 10.0
        {
            self.target_frames -= 1;
            self.stable_hits = 0;
        }
    }
}
