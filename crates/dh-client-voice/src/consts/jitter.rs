/// Duration of one Opus voice frame in milliseconds (`20 ms` = `960` samples @ `48 kHz`).
pub const JITTER_FRAME_DURATION_MS: u32 = 20;

/// Minimum adaptive jitter buffer target depth in frames (`2` frames = `40 ms`).
pub const JITTER_MIN_TARGET_FRAMES: usize = 2;

/// Initial adaptive jitter buffer target depth in frames (`3` frames = `60 ms`).
pub const JITTER_INITIAL_TARGET_FRAMES: usize = 3;

/// Maximum adaptive jitter buffer target depth in frames (`6` frames = `120 ms`).
pub const JITTER_MAX_TARGET_FRAMES: usize = 6;

/// Hard maximum number of buffered frames per speaker (`12` frames = `240 ms`) before dropping
/// oldest packets to prevent unbounded latency buildup.
pub const JITTER_MAX_BUFFERED_FRAMES: usize = 12;

/// Maximum consecutive Opus PLC frames synthesized before cleanly ending the talkspurt (`60 ms`).
pub const JITTER_MAX_PLC_FRAMES: u32 = 3;

/// Per-frame PLC attenuation factors (`[1.0, 0.70, 0.35]`) for smooth fade-out on burst loss.
pub const JITTER_PLC_FADE_GAINS: [f32; 3] = [1.0, 0.70, 0.35];

/// Maximum number of `20 ms` playout frames delivered for one speaker in a single game-thread tick
/// (`5` frames = `100 ms` playout burst cap after frame hitches).
pub const JITTER_MAX_PLAYOUT_FRAMES_PER_TICK: usize = 5;

/// Maximum elapsed tick delta (milliseconds) credited to the playout accumulator per frame (`100 ms`).
pub const JITTER_MAX_TICK_DELTA_MS: u64 = 100;

/// Number of consecutive clean hits before the adaptive jitter target is allowed to shrink by 1 frame.
pub const JITTER_SHRINK_STABLE_FRAMES: u32 = 50;

/// Maximum sequence jump (in frames) treated as forward/reordered rather than a reset (`250` = `5 s`).
pub const JITTER_MAX_REORDER_WINDOW: i16 = 250;

/// Maximum simultaneous remote speaker jitter buffers retained in memory.
pub const JITTER_MAX_SPEAKERS: usize = 64;
