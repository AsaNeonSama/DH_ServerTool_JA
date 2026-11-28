use aec3::audio_processing::aec3::echo_canceller3::EchoCanceller3;
use aec3::audio_processing::audio_buffer::AudioBuffer;
use aec3::audio_processing::stream_config::StreamConfig;
use nnnoiseless::DenoiseState;

use crate::consts::dsp::DSP_FRAME_SAMPLES_10MS;

/// Direct Form II Transposed state for the 2nd-order `80 Hz` Butterworth high-pass filter.
#[derive(Debug, Clone, Copy, Default)]
pub struct BiquadHpfState {
    pub z1: f64,
    pub z2: f64,
}

/// VAD noise gate state with `240 ms` holdover and smooth attack/release envelope.
#[derive(Debug, Clone, Copy)]
pub struct VadGateState {
    /// Remaining `10 ms` holdover frames after the last detected speech frame.
    pub holdover_remaining: u32,
    /// Current smoothed gate envelope gain (`0.0..=1.0`).
    pub envelope_gain: f32,
    /// Whether the gate is currently considered open (transmitting speech or holdover tail).
    pub is_open: bool,
}

/// Smooth Automatic Gain Control + soft-knee limiter state.
#[derive(Debug, Clone, Copy)]
pub struct AgcLimiterState {
    /// Current smoothed linear gain multiplier (`AGC_MIN_GAIN..=AGC_MAX_GAIN`).
    pub current_gain: f32,
}

/// Output of processing a single `10 ms` (`480`-sample @ `48 kHz`) microphone frame through
/// the pure-Rust DSP pipeline.
#[derive(Debug, Clone, Copy)]
pub struct DspFrameResult {
    /// Processed `10 ms` `i16` PCM samples ready to assemble into a `20 ms` Opus frame.
    pub pcm_i16: [i16; DSP_FRAME_SAMPLES_10MS],
    /// Raw `RNNoise` speech probability (`0.0..=1.0`) for diagnostics.
    pub vad_probability: f32,
    /// `true` when the VAD noise gate (including `240 ms` holdover) is open.
    pub speech_active: bool,
}

/// Complete pure-Rust `48 kHz` mono voice capture DSP chain:
/// `80 Hz Butterworth HPF -> WebRTC AEC3 -> RNNoise (nnnoiseless) -> VAD Gate (240 ms holdover) -> Smooth AGC + Soft-Knee Limiter`.
pub struct DspPipeline {
    pub(crate) hpf: BiquadHpfState,
    pub(crate) aec: EchoCanceller3,
    pub(crate) render_buf: AudioBuffer,
    pub(crate) capture_buf: AudioBuffer,
    pub(crate) stream_cfg: StreamConfig,
    pub(crate) denoiser: Box<DenoiseState<'static>>,
    pub(crate) vad: VadGateState,
    pub(crate) agc: AgcLimiterState,
    pub(crate) hpf_scratch: [f32; DSP_FRAME_SAMPLES_10MS],
    pub(crate) rnnoise_scratch: [f32; DSP_FRAME_SAMPLES_10MS],
}
