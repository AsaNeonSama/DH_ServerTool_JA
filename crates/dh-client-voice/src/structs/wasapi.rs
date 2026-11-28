use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

#[cfg(windows)]
use windows::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows::Win32::Media::Audio::WAVEFORMATEX;

use crate::consts::dsp::DSP_FRAME_SAMPLES_10MS;

/// Sample representation parsed from a Windows WASAPI `WAVEFORMATEX` / `WAVEFORMATEXTENSIBLE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasapiSampleFormat {
    /// 32-bit IEEE floating-point samples in `[-1.0, 1.0]`.
    Float32,
    /// 16-bit signed little-endian PCM.
    Int16,
    /// 24-bit signed little-endian PCM (packed 3 bytes or 4-byte container).
    Int24 { container_bytes: usize },
    /// 32-bit signed little-endian PCM.
    Int32,
}

/// Parsed audio stream format negotiated with a WASAPI `IAudioClient`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasapiStreamFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub block_align: u16,
    pub sample_format: WasapiSampleFormat,
}

/// Linear sample-rate converter for non-48 kHz WASAPI endpoints (zero-copy bypass at 48 kHz).
#[derive(Debug, Clone)]
pub struct LinearResampler {
    pub(crate) src_rate: u32,
    pub(crate) dst_rate: u32,
    pub(crate) frac_pos: f64,
    pub(crate) prev_sample: f32,
    pub(crate) has_prev: bool,
}

/// Bounded ring buffer transferring `10 ms` (`480`-sample @ `48 kHz` mono) render-reference
/// frames from the WASAPI loopback thread to the capture/DSP thread.
#[derive(Debug)]
pub struct RenderReferenceRing {
    pub(crate) frames: Mutex<VecDeque<[f32; DSP_FRAME_SAMPLES_10MS]>>,
    pub(crate) capacity: usize,
}

/// Per-frame mutable callback executing exclusively on the WASAPI capture thread (requires neither
/// `Send` nor `Sync` so non-`Send` DSP objects like `EchoCanceller3` stay thread-local with zero locking).
pub type CaptureFrameHandler =
    Box<dyn FnMut(&[f32; DSP_FRAME_SAMPLES_10MS], Option<&[f32; DSP_FRAME_SAMPLES_10MS]>)>;

/// `Send` factory moved once into the WASAPI capture thread to construct the thread-local
/// [`CaptureFrameHandler`] (e.g., `DspPipeline` + `OpusEncoder`).
pub type CaptureFrameFactory = Box<dyn FnOnce() -> CaptureFrameHandler + Send>;

/// Running handle owning the WASAPI capture and loopback background threads.
///
/// Calling [`WasapiSession::stop`] (or dropping) signals both threads to exit and releases
/// both WASAPI audio devices cleanly so Lobby EOS voice has zero contention.
pub struct WasapiSession {
    pub(crate) stop_flag: Arc<AtomicBool>,
    #[cfg(windows)]
    pub(crate) stop_event_raw: usize,
    pub(crate) capture_thread: Option<JoinHandle<()>>,
    pub(crate) loopback_thread: Option<JoinHandle<()>>,
}

/// RAII guard balancing `CoInitializeEx(COINIT_MULTITHREADED)` on a WASAPI worker thread.
#[cfg(windows)]
pub(crate) struct ComInitGuard {
    pub(crate) should_uninit: bool,
}

/// RAII guard freeing a `WAVEFORMATEX*` allocated by `IAudioClient::GetMixFormat`.
#[cfg(windows)]
pub(crate) struct CoTaskMemWaveFormatGuard {
    pub(crate) ptr: *mut WAVEFORMATEX,
}

/// RAII guard owning a Win32 event `HANDLE` created via `CreateEventW`.
#[cfg(windows)]
pub(crate) struct OwnedEventHandle(pub(crate) HANDLE);
