/// Voice engine internal sample rate in Hz (`48 kHz` matching UE4 `USoundWaveProcedural`,
/// `libopus` fullband, WebRTC `aec3`, and `nnnoiseless` `RNNoise`).
pub const VOICE_SAMPLE_RATE_HZ: u32 = 48_000;

/// Sample count for a single `10 ms` DSP processing frame at `48 kHz` mono (`480` samples).
pub const DSP_FRAME_SAMPLES_10MS: usize = 480;

/// Sample count for a single `20 ms` Opus packet frame at `48 kHz` mono (`960` samples).
pub const OPUS_FRAME_SAMPLES_20MS: usize = 960;

/// Full-scale signed 16-bit PCM amplitude used to convert between normalized `[-1.0, 1.0]`
/// float PCM and WebRTC/RNNoise `[-32768.0, 32767.0]` float PCM.
pub const PCM_S16_SCALE: f32 = 32_768.0;

/// Inverse of [`PCM_S16_SCALE`] (`1.0 / 32768.0`).
pub const PCM_S16_INV_SCALE: f32 = 1.0 / 32_768.0;

/// 2nd-order Butterworth high-pass filter coefficients for `fc = 80 Hz` at `Fs = 48,000 Hz`.
/// Computed via bilinear transform (`Q = 1 / sqrt(2)`): exact DC notch, unity Nyquist passband.
pub const HPF_80HZ_B0: f64 = 0.992_622_544_432_305_3;
pub const HPF_80HZ_B1: f64 = -1.985_245_088_864_610_7;
pub const HPF_80HZ_B2: f64 = 0.992_622_544_432_305_3;
pub const HPF_80HZ_A1: f64 = -1.985_190_665_107_154;
pub const HPF_80HZ_A2: f64 = 0.985_299_512_622_067_3;

/// Initial AEC3 audio buffer delay estimate in milliseconds (Windows WASAPI shared-mode typical).
pub const AEC3_INITIAL_DELAY_MS: i32 = 40;

/// Minimum `RNNoise` speech probability (`0.0..=1.0`) required to open the VAD noise gate.
pub const VAD_OPEN_PROB_THRESHOLD: f32 = 0.35;

/// Minimum post-RNNoise normalized RMS required alongside VAD probability to avoid opening on
/// digital zero/near-zero floor noise (`~ -54 dBFS`).
pub const VAD_MIN_RMS_THRESHOLD: f32 = 0.002;

/// Number of `10 ms` frames the VAD gate remains open after speech ends (`240 ms` holdover).
pub const VAD_HOLDOVER_FRAMES: u32 = 24;

/// Per-frame (`10 ms`) exponential attack coefficient when opening the VAD gate (fast, ~15 ms).
pub const VAD_GATE_ATTACK_ALPHA: f32 = 0.65;

/// Per-frame (`10 ms`) exponential release coefficient when closing the VAD gate (smooth, ~80 ms).
pub const VAD_GATE_RELEASE_ALPHA: f32 = 0.18;

/// Residual gate gain below which the frame is treated as completely gated/silent.
pub const VAD_GATE_SILENCE_EPSILON: f32 = 0.005;

/// Target normalized speech RMS level for smooth AGC (`~ -19 dBFS`).
pub const AGC_TARGET_RMS: f32 = 0.11;

/// Minimum AGC gain multiplier (`-6 dB`).
pub const AGC_MIN_GAIN: f32 = 0.5;

/// Maximum AGC gain multiplier (`+12 dB`, prevents amplifying background hiss).
pub const AGC_MAX_GAIN: f32 = 4.0;

/// Minimum RMS above which AGC adapts gain (avoids boosting silence during holdover tails).
pub const AGC_ADAPT_MIN_RMS: f32 = 0.004;

/// Smooth AGC gain decrease coefficient (`10 ms` frame, responds quickly to loud shouting).
pub const AGC_ATTACK_ALPHA: f32 = 0.25;

/// Smooth AGC gain increase coefficient (`10 ms` frame, rises gently without pumping).
pub const AGC_RELEASE_ALPHA: f32 = 0.04;

/// Soft-knee limiter onset threshold (normalized amplitude `~ -1.4 dBFS`).
pub const LIMITER_KNEE_THRESHOLD: f32 = 0.85;

/// Soft-knee limiter hard ceiling (normalized amplitude `~ -0.18 dBFS`).
pub const LIMITER_CEILING: f32 = 0.98;
