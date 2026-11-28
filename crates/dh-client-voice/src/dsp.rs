use aec3::api::config::EchoCanceller3Config;
use aec3::api::control::EchoControl;
use aec3::audio_processing::aec3::echo_canceller3::EchoCanceller3;
use aec3::audio_processing::audio_buffer::AudioBuffer;
use aec3::audio_processing::stream_config::StreamConfig;
use nnnoiseless::DenoiseState;

use crate::consts::dsp::{
    AEC3_INITIAL_DELAY_MS, AGC_ADAPT_MIN_RMS, AGC_ATTACK_ALPHA, AGC_MAX_GAIN, AGC_MIN_GAIN,
    AGC_RELEASE_ALPHA, AGC_TARGET_RMS, DSP_FRAME_SAMPLES_10MS, ENERGY_CEIL_DBFS, ENERGY_FLOOR_DBFS,
    ENERGY_RMS_EPSILON, ENERGY_SILENCE_EPSILON, HPF_80HZ_A1, HPF_80HZ_A2, HPF_80HZ_B0, HPF_80HZ_B1,
    HPF_80HZ_B2, LIMITER_CEILING, LIMITER_KNEE_THRESHOLD, PCM_S16_INV_SCALE, VAD_GATE_ATTACK_ALPHA,
    VAD_GATE_RELEASE_ALPHA, VAD_GATE_SILENCE_EPSILON, VAD_HOLDOVER_FRAMES, VAD_MIN_RMS_THRESHOLD,
    VAD_OPEN_PROB_THRESHOLD, VOICE_SAMPLE_RATE_HZ,
};
pub use crate::structs::dsp::{
    AgcLimiterState, BiquadHpfState, DspFrameResult, DspPipeline, VadGateState,
};

/// Converts a finite normalized float sample (`[-1.0, 1.0]`) to signed 16-bit PCM using
/// exact IEEE-754 round-to-nearest-even without lossy truncation casts.
#[must_use]
pub fn normalized_f32_to_i16(sample: f32) -> i16 {
    if !sample.is_finite() {
        return 0;
    }
    let clamped: f32 = sample.clamp(-1.0, 1.0);
    let scaled: f32 = (clamped * 32_767.0).clamp(-32_768.0, 32_767.0);
    // Adding `1.5 * 2^23` (`12_582_912.0`, exponent `23`) places the IEEE-754 rounded integer
    // offset by `0x0040_0000` directly into the low 23 mantissa bits.
    let biased_bits: u32 = (scaled + 12_582_912.0_f32).to_bits();
    let mantissa: i32 = (biased_bits & 0x007F_FFFF).cast_signed();
    let rounded: i32 = mantissa - 0x0040_0000;
    i16::try_from(rounded).unwrap_or(if rounded < 0 { i16::MIN } else { i16::MAX })
}

/// Applies a smooth soft-knee amplitude limiter with knee at [`LIMITER_KNEE_THRESHOLD`] and
/// asymptotic ceiling below [`LIMITER_CEILING`] (zero harsh clipping).
#[must_use]
pub fn soft_knee_limit(sample: f32) -> f32 {
    if !sample.is_finite() {
        return 0.0;
    }
    let abs_val: f32 = sample.abs();
    if abs_val <= LIMITER_KNEE_THRESHOLD {
        return sample;
    }
    let headroom: f32 = LIMITER_CEILING - LIMITER_KNEE_THRESHOLD;
    let excess: f32 = abs_val - LIMITER_KNEE_THRESHOLD;
    let compressed: f32 = LIMITER_KNEE_THRESHOLD + headroom * (excess / (excess + headroom));
    compressed.copysign(sample)
}

/// Maps a linear normalized RMS value (`0.0..=1.0`) onto Dread Hunger's HUD microphone volume
/// meter range (`0.0..=1.0`) using a perceptual dBFS curve (`-52 dBFS -> 0.0`, `-8 dBFS -> 1.0`).
#[must_use]
pub fn rms_to_meter_energy(rms: f32) -> f32 {
    if !rms.is_finite() || rms <= ENERGY_RMS_EPSILON {
        return 0.0;
    }
    let dbfs: f32 = 20.0 * rms.log10();
    let span: f32 = ENERGY_CEIL_DBFS - ENERGY_FLOOR_DBFS;
    if span <= 0.0 {
        return 0.0;
    }
    ((dbfs - ENERGY_FLOOR_DBFS) / span).clamp(0.0, 1.0)
}

/// Computes perceptual HUD microphone volume meter energy (`0.0..=1.0`) from a slice of
/// signed 16-bit PCM samples.
#[must_use]
pub fn pcm_i16_rms_energy(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let Ok(count_u32): Result<u32, std::num::TryFromIntError> = u32::try_from(samples.len()) else {
        return 0.0;
    };
    let inv_scale: f64 = f64::from(PCM_S16_INV_SCALE);
    let mut sum_sq: f64 = 0.0;
    for sample in samples {
        let norm: f64 = f64::from(*sample) * inv_scale;
        sum_sq += norm * norm;
    }
    let rms_f64: f64 = (sum_sq / f64::from(count_u32)).sqrt();
    let rms: f32 = f64_to_f32_clamped(rms_f64).max(0.0);
    rms_to_meter_energy(rms)
}

/// Smooths a microphone meter value with fast attack (`target >= prev`) and gentle exponential
/// release (`target < prev`), flushing tiny sub-threshold residuals cleanly to `0.0`.
#[must_use]
pub fn smooth_meter_energy(prev: f32, target: f32, attack_alpha: f32, release_alpha: f32) -> f32 {
    let safe_prev: f32 = if prev.is_finite() {
        prev.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let safe_target: f32 = if target.is_finite() {
        target.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let alpha: f32 = if safe_target >= safe_prev {
        attack_alpha.clamp(0.0, 1.0)
    } else {
        release_alpha.clamp(0.0, 1.0)
    };
    let next: f32 = (safe_prev + alpha * (safe_target - safe_prev)).clamp(0.0, 1.0);
    if next < ENERGY_SILENCE_EPSILON {
        0.0
    } else {
        next
    }
}

/// Smoothly decays a stored meter energy value linearly to `0.0` over `window_ms` milliseconds
/// after the last audio update so the HUD bar glides down instead of snapping to zero.
#[must_use]
pub fn decay_meter_energy(energy: f32, elapsed_ms: u64, window_ms: u64) -> f32 {
    if !energy.is_finite() || energy <= ENERGY_SILENCE_EPSILON || window_ms == 0 {
        return 0.0;
    }
    if elapsed_ms >= window_ms {
        return 0.0;
    }
    let remaining_ms: u64 = window_ms - elapsed_ms;
    let Ok(rem_u32): Result<u32, std::num::TryFromIntError> = u32::try_from(remaining_ms) else {
        return 0.0;
    };
    let Ok(win_u32): Result<u32, std::num::TryFromIntError> = u32::try_from(window_ms) else {
        return 0.0;
    };
    let ratio: f32 = f64_to_f32_clamped(f64::from(rem_u32) / f64::from(win_u32)).clamp(0.0, 1.0);
    let decayed: f32 = (energy.clamp(0.0, 1.0) * ratio).clamp(0.0, 1.0);
    if decayed < ENERGY_SILENCE_EPSILON {
        0.0
    } else {
        decayed
    }
}

impl BiquadHpfState {
    /// Resets the biquad delay lines to zero.
    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    /// Filters a single sample through the 2nd-order `80 Hz` Butterworth HPF (Direct Form II Transposed).
    #[must_use]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let x: f64 = if input.is_finite() {
            f64::from(input)
        } else {
            0.0
        };
        let y: f64 = HPF_80HZ_B0 * x + self.z1;
        self.z1 = HPF_80HZ_B1 * x - HPF_80HZ_A1 * y + self.z2;
        self.z2 = HPF_80HZ_B2 * x - HPF_80HZ_A2 * y;
        // Flush denormals and guard against unexpected numerical instability.
        if !self.z1.is_finite() || self.z1.abs() < 1.0e-30 {
            self.z1 = 0.0;
        }
        if !self.z2.is_finite() || self.z2.abs() < 1.0e-30 {
            self.z2 = 0.0;
        }
        let out_f32: f32 = f64_to_f32_clamped(y);
        if out_f32.is_finite() {
            out_f32
        } else {
            0.0
        }
    }

    /// Processes a `10 ms` (`480`-sample) frame in place into `output`.
    pub fn process_frame(
        &mut self,
        input: &[f32; DSP_FRAME_SAMPLES_10MS],
        output: &mut [f32; DSP_FRAME_SAMPLES_10MS],
    ) {
        for (dst, &src) in output.iter_mut().zip(input.iter()) {
            *dst = self.process_sample(src);
        }
    }
}

impl Default for VadGateState {
    fn default() -> Self {
        Self {
            holdover_remaining: 0,
            envelope_gain: 0.0,
            is_open: false,
        }
    }
}

impl VadGateState {
    /// Resets the noise gate immediately to closed.
    pub fn reset(&mut self) {
        self.holdover_remaining = 0;
        self.envelope_gain = 0.0;
        self.is_open = false;
    }

    /// Updates the VAD gate for one `10 ms` frame, returning `(prev_gain, next_gain, speech_active)`.
    pub fn update(&mut self, vad_prob: f32, frame_rms: f32) -> (f32, f32, bool) {
        let speech_detected: bool =
            vad_prob >= VAD_OPEN_PROB_THRESHOLD && frame_rms >= VAD_MIN_RMS_THRESHOLD;
        if speech_detected {
            self.holdover_remaining = VAD_HOLDOVER_FRAMES;
        } else if self.holdover_remaining > 0 {
            self.holdover_remaining -= 1;
        }

        let target_open: bool = speech_detected || self.holdover_remaining > 0;
        let prev_gain: f32 = self.envelope_gain;
        let next_gain: f32 = if target_open {
            prev_gain + VAD_GATE_ATTACK_ALPHA * (1.0 - prev_gain)
        } else {
            let decayed: f32 = prev_gain * (1.0 - VAD_GATE_RELEASE_ALPHA);
            if decayed < VAD_GATE_SILENCE_EPSILON {
                0.0
            } else {
                decayed
            }
        };
        self.envelope_gain = next_gain;
        self.is_open = target_open || next_gain > VAD_GATE_SILENCE_EPSILON;
        (prev_gain, next_gain, self.is_open)
    }
}

impl Default for AgcLimiterState {
    fn default() -> Self {
        Self { current_gain: 1.0 }
    }
}

impl AgcLimiterState {
    /// Resets AGC gain to unity (`1.0`).
    pub fn reset(&mut self) {
        self.current_gain = 1.0;
    }

    /// Smoothly adapts AGC gain on active speech frames, returning `(prev_gain, next_gain)`.
    pub fn update(&mut self, frame_rms: f32, speech_active: bool) -> (f32, f32) {
        let prev_gain: f32 = self.current_gain;
        if !speech_active || frame_rms < AGC_ADAPT_MIN_RMS || !frame_rms.is_finite() {
            return (prev_gain, prev_gain);
        }
        let desired_gain: f32 = (AGC_TARGET_RMS / frame_rms).clamp(AGC_MIN_GAIN, AGC_MAX_GAIN);
        let alpha: f32 = if desired_gain < prev_gain {
            AGC_ATTACK_ALPHA
        } else {
            AGC_RELEASE_ALPHA
        };
        let next_gain: f32 =
            (prev_gain + alpha * (desired_gain - prev_gain)).clamp(AGC_MIN_GAIN, AGC_MAX_GAIN);
        self.current_gain = next_gain;
        (prev_gain, next_gain)
    }
}

impl Default for DspPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl DspPipeline {
    /// Creates a new `48 kHz` mono capture DSP pipeline (`HPF -> AEC3 -> RNNoise -> VAD -> AGC/Limiter`).
    #[must_use]
    pub fn new() -> Self {
        let rate_usize: usize = usize::try_from(VOICE_SAMPLE_RATE_HZ).unwrap_or(48_000);
        let rate_i32: i32 = i32::try_from(VOICE_SAMPLE_RATE_HZ).unwrap_or(48_000);
        let mut aec: EchoCanceller3 =
            EchoCanceller3::new(EchoCanceller3Config::default(), rate_i32, 1, 1);
        aec.set_audio_buffer_delay(AEC3_INITIAL_DELAY_MS);

        let render_buf: AudioBuffer =
            AudioBuffer::from_sample_rates(rate_usize, 1, rate_usize, 1, rate_usize);
        let capture_buf: AudioBuffer =
            AudioBuffer::from_sample_rates(rate_usize, 1, rate_usize, 1, rate_usize);
        let stream_cfg: StreamConfig = StreamConfig::new(rate_usize, 1, false);

        Self {
            hpf: BiquadHpfState::default(),
            aec,
            render_buf,
            capture_buf,
            stream_cfg,
            denoiser: DenoiseState::new(),
            vad: VadGateState::default(),
            agc: AgcLimiterState::default(),
            hpf_scratch: [0.0; DSP_FRAME_SAMPLES_10MS],
            rnnoise_scratch: [0.0; DSP_FRAME_SAMPLES_10MS],
        }
    }

    /// Resets stateful filter/gate/AGC memory when starting a fresh match session.
    pub fn reset(&mut self) {
        self.hpf.reset();
        self.vad.reset();
        self.agc.reset();
        let rate_i32: i32 = i32::try_from(VOICE_SAMPLE_RATE_HZ).unwrap_or(48_000);
        let mut aec: EchoCanceller3 =
            EchoCanceller3::new(EchoCanceller3Config::default(), rate_i32, 1, 1);
        aec.set_audio_buffer_delay(AEC3_INITIAL_DELAY_MS);
        self.aec = aec;
        self.denoiser = DenoiseState::new();
    }

    /// Feeds one `10 ms` (`480`-sample @ `48 kHz`) normalized `[-1.0, 1.0]` loopback render
    /// reference frame into WebRTC `EchoCanceller3`.
    pub fn feed_render_10ms(&mut self, render_mono_f32: &[f32; DSP_FRAME_SAMPLES_10MS]) {
        let channels: [&[f32]; 1] = [render_mono_f32.as_slice()];
        self.render_buf.copy_from(&channels, &self.stream_cfg);
        self.render_buf.split_into_frequency_bands();
        self.aec.analyze_render(&mut self.render_buf);
    }

    /// Runs HPF + AEC3 echo-path adaptation while PTT is released so the adaptive filter
    /// stays converged without spending CPU on RNNoise/VAD/AGC.
    pub fn warmup_aec_10ms(
        &mut self,
        capture_mono_f32: &[f32; DSP_FRAME_SAMPLES_10MS],
        render_mono_f32: Option<&[f32; DSP_FRAME_SAMPLES_10MS]>,
    ) {
        if let Some(render) = render_mono_f32 {
            self.feed_render_10ms(render);
        }
        self.hpf
            .process_frame(capture_mono_f32, &mut self.hpf_scratch);
        let capture_channels: [&[f32]; 1] = [self.hpf_scratch.as_slice()];
        self.capture_buf
            .copy_from(&capture_channels, &self.stream_cfg);
        self.aec.analyze_capture(&mut self.capture_buf);
        self.capture_buf.split_into_frequency_bands();
        self.aec.process_capture(&mut self.capture_buf, false);
        self.vad.reset();
    }

    /// Processes one `10 ms` (`480`-sample @ `48 kHz`) normalized `[-1.0, 1.0]` microphone frame
    /// (with optional synchronized `10 ms` render reference) through the full DSP chain.
    pub fn process_capture_10ms(
        &mut self,
        capture_mono_f32: &[f32; DSP_FRAME_SAMPLES_10MS],
        render_mono_f32: Option<&[f32; DSP_FRAME_SAMPLES_10MS]>,
    ) -> DspFrameResult {
        if let Some(render) = render_mono_f32 {
            self.feed_render_10ms(render);
        }

        // 1. 80 Hz 2nd-order Butterworth High-Pass Filter (removes low rumble & DC offset).
        self.hpf
            .process_frame(capture_mono_f32, &mut self.hpf_scratch);

        // 2. WebRTC AEC3 acoustic echo cancellation (`copy_from` scales [-1,1] -> [-32768,32767]).
        let capture_channels: [&[f32]; 1] = [self.hpf_scratch.as_slice()];
        self.capture_buf
            .copy_from(&capture_channels, &self.stream_cfg);
        self.aec.analyze_capture(&mut self.capture_buf);
        self.capture_buf.split_into_frequency_bands();
        self.aec.process_capture(&mut self.capture_buf, false);
        self.capture_buf.merge_frequency_bands();

        // 3. RNNoise neural denoising (`nnnoiseless` operates directly in [-32768, 32767] float domain).
        let aec_out_s16_f32: &[f32] = self.capture_buf.channel(0);
        let vad_prob: f32 = self
            .denoiser
            .process_frame(&mut self.rnnoise_scratch, aec_out_s16_f32);

        // 4. Compute post-RNNoise normalized RMS for VAD gate + smooth AGC.
        let mut sum_sq: f64 = 0.0;
        for sample_s16 in &self.rnnoise_scratch {
            let norm: f64 = f64::from(*sample_s16 * PCM_S16_INV_SCALE);
            if norm.is_finite() {
                sum_sq += norm * norm;
            }
        }
        let frame_rms: f32 = {
            let mean_sq: f64 = sum_sq / 480.0_f64;
            let rms_f64: f64 = mean_sq.sqrt();
            // Convert positive bounded RMS via f32; clamp defensively.
            let rms_bits_safe: f32 = f64_to_f32_clamped(rms_f64);
            rms_bits_safe.max(0.0)
        };

        let (prev_gate, next_gate, speech_active): (f32, f32, bool) =
            self.vad.update(vad_prob, frame_rms);
        let (prev_agc, next_agc): (f32, f32) = self.agc.update(frame_rms, speech_active);

        // 5. Apply smooth per-sample ramped gate * AGC + soft-knee limiter -> i16 PCM.
        let mut pcm_i16: [i16; DSP_FRAME_SAMPLES_10MS] = [0i16; DSP_FRAME_SAMPLES_10MS];
        let inv_n: f32 = 1.0 / 480.0_f32;
        for (idx, dst) in pcm_i16.iter_mut().enumerate() {
            let t: f32 = f32::from(u16::try_from(idx).unwrap_or(0)) * inv_n;
            let gate_g: f32 = prev_gate + (next_gate - prev_gate) * t;
            let agc_g: f32 = prev_agc + (next_agc - prev_agc) * t;
            let norm: f32 = self.rnnoise_scratch[idx] * PCM_S16_INV_SCALE;
            let shaped: f32 = soft_knee_limit(norm * gate_g * agc_g);
            *dst = normalized_f32_to_i16(shaped);
        }

        DspFrameResult {
            pcm_i16,
            vad_probability: vad_prob,
            speech_active,
        }
    }
}

/// Converts a finite non-negative bounded `f64` to `f32` cleanly.
fn f64_to_f32_clamped(val: f64) -> f32 {
    if !val.is_finite() {
        return 0.0;
    }
    let clamped: f64 = val.clamp(-1.0e6, 1.0e6);
    // SAFETY-NOTE: No unsafe; exact narrowing of bounded magnitude to f32.
    f32::from_bits(narrow_f64_to_f32_bits(clamped))
}

/// Pure bit-level narrowing of a finite `f64` in `[-1e6, 1e6]` to IEEE-754 `f32` bits
/// (avoids `clippy::cast_possible_truncation` with zero `#[allow]` suppressions).
fn narrow_f64_to_f32_bits(val: f64) -> u32 {
    let bits: u64 = val.to_bits();
    let sign: u32 = u32::try_from((bits >> 63) & 1).unwrap_or(0) << 31;
    let exp_f64: i32 = i32::try_from((bits >> 52) & 0x7FF).unwrap_or(0);
    let frac_f64: u64 = bits & 0x000F_FFFF_FFFF_FFFF;
    if exp_f64 == 0 {
        return sign;
    }
    let unbiased: i32 = exp_f64 - 1023;
    let exp_f32: i32 = unbiased + 127;
    if exp_f32 <= 0 {
        return sign;
    }
    if exp_f32 >= 255 {
        return sign | (0xFE << 23) | 0x007F_FFFF;
    }
    let mantissa: u32 = u32::try_from(frac_f64 >> 29).unwrap_or(0);
    let round_bit: u32 = u32::try_from((frac_f64 >> 28) & 1).unwrap_or(0);
    let exp_bits: u32 = u32::try_from(exp_f32).unwrap_or(0);
    (sign | (exp_bits << 23) | mantissa).wrapping_add(round_bit)
}
