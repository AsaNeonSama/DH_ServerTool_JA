use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::consts::dsp::{DSP_FRAME_SAMPLES_10MS, VOICE_SAMPLE_RATE_HZ};
use crate::consts::wasapi::{
    HRESULT_AUDCLNT_E_DEVICE_INVALIDATED, HRESULT_AUDCLNT_E_RESOURCES_INVALIDATED,
    HRESULT_AUDCLNT_E_SERVICE_NOT_RUNNING, SUBTYPE_IEEE_FLOAT_DATA1, SUBTYPE_PCM_DATA1,
    WASAPI_BUFFER_DURATION_HNS, WASAPI_CAPTURE_WAIT_MS, WASAPI_LOOPBACK_POLL_MS,
    WASAPI_MAX_ACCUMULATOR_SAMPLES, WASAPI_RECONNECT_BACKOFF_MS,
    WASAPI_RENDER_RING_CAPACITY_FRAMES, WAVE_FORMAT_EXTENSIBLE_TAG, WAVE_FORMAT_IEEE_FLOAT_TAG,
    WAVE_FORMAT_PCM_TAG,
};
use crate::logger::log_line;
pub use crate::structs::wasapi::{
    CaptureFrameFactory, CaptureFrameHandler, LinearResampler, RenderReferenceRing,
    WasapiSampleFormat, WasapiSession, WasapiStreamFormat,
};
#[cfg(windows)]
use crate::structs::wasapi::{CoTaskMemWaveFormatGuard, ComInitGuard, OwnedEventHandle};

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
#[cfg(windows)]
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDevice,
    IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX,
    WAVEFORMATEXTENSIBLE,
};
#[cfg(windows)]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    CreateEventW, SetEvent, WaitForMultipleObjects, WaitForSingleObject,
};

/// Returns `true` if `hresult` indicates a WASAPI endpoint hot-plug / audio service reset
/// that should trigger automatic device re-enumeration.
#[must_use]
pub fn is_recoverable_device_error(hresult: i32) -> bool {
    hresult == HRESULT_AUDCLNT_E_DEVICE_INVALIDATED
        || hresult == HRESULT_AUDCLNT_E_SERVICE_NOT_RUNNING
        || hresult == HRESULT_AUDCLNT_E_RESOURCES_INVALIDATED
}

impl LinearResampler {
    /// Creates a linear resampler converting mono `f32` samples from `src_rate` to `dst_rate`.
    #[must_use]
    pub fn new(src_rate: u32, dst_rate: u32) -> Self {
        Self {
            src_rate: src_rate.max(1),
            dst_rate: dst_rate.max(1),
            frac_pos: 0.0,
            prev_sample: 0.0,
            has_prev: false,
        }
    }

    /// Resamples `input` mono samples and appends the resulting `dst_rate` samples to `out`.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if input.is_empty() {
            return;
        }
        if self.src_rate == self.dst_rate {
            out.extend_from_slice(input);
            return;
        }

        let step: f64 = f64::from(self.src_rate) / f64::from(self.dst_rate);
        let mut prev: f32 = if self.has_prev {
            self.prev_sample
        } else {
            self.has_prev = true;
            input[0]
        };

        for &curr in input {
            while self.frac_pos < 1.0 {
                let frac_f32: f32 = frac_f64_unit_to_f32(self.frac_pos);
                let interp: f32 = prev + (curr - prev) * frac_f32;
                out.push(interp);
                self.frac_pos += step;
            }
            self.frac_pos -= 1.0;
            prev = curr;
        }
        self.prev_sample = prev;
    }
}

/// Converts a unit-interval `f64` in `[0.0, 1.0]` to `f32` cleanly without truncation lints.
fn frac_f64_unit_to_f32(val: f64) -> f32 {
    let clamped: f64 = val.clamp(0.0, 1.0);
    let bits: u64 = clamped.to_bits();
    let exp_f64: i32 = i32::try_from((bits >> 52) & 0x7FF).unwrap_or(0);
    let frac_f64: u64 = bits & 0x000F_FFFF_FFFF_FFFF;
    if exp_f64 == 0 {
        return 0.0;
    }
    let exp_f32: i32 = (exp_f64 - 1023) + 127;
    if exp_f32 <= 0 {
        return 0.0;
    }
    if exp_f32 >= 127 {
        return 1.0;
    }
    let mantissa: u32 = u32::try_from(frac_f64 >> 29).unwrap_or(0);
    let exp_bits: u32 = u32::try_from(exp_f32).unwrap_or(0);
    f32::from_bits((exp_bits << 23) | mantissa)
}

impl RenderReferenceRing {
    /// Creates a bounded loopback render-reference ring holding up to `capacity` `10 ms` frames.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let cap: usize = capacity.max(2);
        Self {
            frames: Mutex::new(VecDeque::with_capacity(cap)),
            capacity: cap,
        }
    }

    /// Pushes one `10 ms` render reference frame, dropping the oldest frame if full.
    pub fn push_frame(&self, frame: [f32; DSP_FRAME_SAMPLES_10MS]) {
        if let Ok(mut guard) = self.frames.lock() {
            while guard.len() >= self.capacity {
                let _old: Option<[f32; DSP_FRAME_SAMPLES_10MS]> = guard.pop_front();
            }
            guard.push_back(frame);
        }
    }

    /// Pops the oldest queued `10 ms` render reference frame for AEC3 analysis.
    #[must_use]
    pub fn pop_frame(&self) -> Option<[f32; DSP_FRAME_SAMPLES_10MS]> {
        self.frames.lock().ok().and_then(
            |mut guard: std::sync::MutexGuard<'_, VecDeque<[f32; DSP_FRAME_SAMPLES_10MS]>>| {
                guard.pop_front()
            },
        )
    }

    /// Clears all queued render reference frames.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.frames.lock() {
            guard.clear();
        }
    }
}

/// Decodes interleaved WASAPI raw PCM bytes according to `fmt` and downmixes all channels to
/// normalized mono `f32` samples in `[-1.0, 1.0]`.
pub fn decode_interleaved_to_mono_f32(
    raw: &[u8],
    frame_count: usize,
    fmt: WasapiStreamFormat,
    out_mono: &mut Vec<f32>,
) {
    let channels: usize = usize::from(fmt.channels);
    let block_align: usize = usize::from(fmt.block_align);
    if channels == 0 || block_align == 0 || frame_count == 0 {
        return;
    }
    let inv_channels: f32 = 1.0 / f32::from(fmt.channels);
    let sample_stride: usize = match fmt.sample_format {
        WasapiSampleFormat::Float32 | WasapiSampleFormat::Int32 => 4,
        WasapiSampleFormat::Int16 => 2,
        WasapiSampleFormat::Int24 { container_bytes } => container_bytes,
    };
    if sample_stride == 0 || channels.saturating_mul(sample_stride) > block_align {
        return;
    }

    out_mono.reserve(frame_count);
    for frame_idx in 0..frame_count {
        let Some(frame_off): Option<usize> = frame_idx.checked_mul(block_align) else {
            break;
        };
        let Some(frame_end): Option<usize> = frame_off.checked_add(block_align) else {
            break;
        };
        let Some(frame_bytes): Option<&[u8]> = raw.get(frame_off..frame_end) else {
            break;
        };

        let mut acc: f32 = 0.0;
        for ch in 0..channels {
            let off: usize = ch * sample_stride;
            let val: f32 = match fmt.sample_format {
                WasapiSampleFormat::Float32 => {
                    let s: f32 = f32::from_le_bytes([
                        frame_bytes[off],
                        frame_bytes[off + 1],
                        frame_bytes[off + 2],
                        frame_bytes[off + 3],
                    ]);
                    if s.is_finite() {
                        s.clamp(-1.0, 1.0)
                    } else {
                        0.0
                    }
                }
                WasapiSampleFormat::Int16 => {
                    let s: i16 = i16::from_le_bytes([frame_bytes[off], frame_bytes[off + 1]]);
                    f32::from(s) / 32_768.0_f32
                }
                WasapiSampleFormat::Int24 { container_bytes } => {
                    // 24-bit signed little-endian (either 3 packed bytes or 4-byte container in high 24 bits).
                    let b: [u8; 4] = if container_bytes == 4 {
                        [
                            frame_bytes[off],
                            frame_bytes[off + 1],
                            frame_bytes[off + 2],
                            frame_bytes[off + 3],
                        ]
                    } else {
                        [
                            0,
                            frame_bytes[off],
                            frame_bytes[off + 1],
                            frame_bytes[off + 2],
                        ]
                    };
                    let shifted: i32 = i32::from_le_bytes(b) >> 8;
                    f32::from(i16::try_from(shifted >> 8).unwrap_or(0)) / 32_768.0_f32
                }
                WasapiSampleFormat::Int32 => {
                    let s: i32 = i32::from_le_bytes([
                        frame_bytes[off],
                        frame_bytes[off + 1],
                        frame_bytes[off + 2],
                        frame_bytes[off + 3],
                    ]);
                    f32::from(i16::try_from(s >> 16).unwrap_or(0)) / 32_768.0_f32
                }
            };
            acc += val;
        }
        out_mono.push((acc * inv_channels).clamp(-1.0, 1.0));
    }
}

#[cfg(windows)]
impl ComInitGuard {
    fn init_mta() -> Self {
        // SAFETY: Initializes COM in multithreaded apartment mode on this dedicated worker thread.
        let hr: windows::core::HRESULT = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self {
            should_uninit: hr.is_ok(),
        }
    }
}

#[cfg(windows)]
impl Drop for ComInitGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            // SAFETY: Balances the successful `CoInitializeEx` on this same worker thread.
            unsafe {
                CoUninitialize();
            }
        }
    }
}

#[cfg(windows)]
impl Drop for CoTaskMemWaveFormatGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: `self.ptr` was allocated by COM `IAudioClient::GetMixFormat`.
            unsafe {
                CoTaskMemFree(Some(self.ptr.cast::<c_void>()));
            }
            self.ptr = std::ptr::null_mut();
        }
    }
}

#[cfg(windows)]
impl Drop for OwnedEventHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: `self.0` is an owned Win32 event handle created by `CreateEventW`.
            unsafe {
                let _closed: Result<(), windows::core::Error> = CloseHandle(self.0);
            }
            self.0 = HANDLE::default();
        }
    }
}

#[cfg(windows)]
unsafe fn parse_wave_format(fmt_ptr: *const WAVEFORMATEX) -> Option<WasapiStreamFormat> {
    if fmt_ptr.is_null() {
        return None;
    }
    // SAFETY: `fmt_ptr` is a valid `WAVEFORMATEX` returned by `IAudioClient::GetMixFormat`.
    let base: WAVEFORMATEX = unsafe { std::ptr::read_unaligned(fmt_ptr) };
    let sample_rate: u32 = base.nSamplesPerSec;
    let channels: u16 = base.nChannels;
    let block_align: u16 = base.nBlockAlign;
    let bits: u16 = base.wBitsPerSample;
    let tag: u16 = base.wFormatTag;

    let sample_format: WasapiSampleFormat = match tag {
        WAVE_FORMAT_IEEE_FLOAT_TAG if bits == 32 => WasapiSampleFormat::Float32,
        WAVE_FORMAT_PCM_TAG if bits == 16 => WasapiSampleFormat::Int16,
        WAVE_FORMAT_PCM_TAG if bits == 24 => WasapiSampleFormat::Int24 { container_bytes: 3 },
        WAVE_FORMAT_PCM_TAG if bits == 32 => WasapiSampleFormat::Int32,
        WAVE_FORMAT_EXTENSIBLE_TAG => {
            if usize::from(base.cbSize) < 22 {
                return None;
            }
            // SAFETY: `WAVE_FORMAT_EXTENSIBLE` begins with `WAVEFORMATEX` and `cbSize >= 22`.
            let ext: WAVEFORMATEXTENSIBLE =
                unsafe { std::ptr::read_unaligned(fmt_ptr.cast::<WAVEFORMATEXTENSIBLE>()) };
            let sub_data1: u32 = ext.SubFormat.data1;
            // SAFETY: `Samples` union holds `wValidBitsPerSample` for PCM/float extensible formats.
            let valid_bits: u16 = unsafe { ext.Samples.wValidBitsPerSample };
            if sub_data1 == SUBTYPE_IEEE_FLOAT_DATA1 && bits == 32 {
                WasapiSampleFormat::Float32
            } else if sub_data1 == SUBTYPE_PCM_DATA1 {
                match (bits, valid_bits) {
                    (16, _) => WasapiSampleFormat::Int16,
                    (24, _) => WasapiSampleFormat::Int24 { container_bytes: 3 },
                    (32, 24) => WasapiSampleFormat::Int24 { container_bytes: 4 },
                    (32, _) => WasapiSampleFormat::Int32,
                    _ => return None,
                }
            } else {
                return None;
            }
        }
        _ => return None,
    };

    Some(WasapiStreamFormat {
        sample_rate,
        channels,
        block_align,
        sample_format,
    })
}

#[cfg(windows)]
fn open_default_device(
    enumerator: &IMMDeviceEnumerator,
    is_loopback: bool,
) -> Result<IMMDevice, windows::core::Error> {
    if is_loopback {
        // SAFETY: Querying default Windows render endpoint for AEC3 loopback reference.
        unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
    } else {
        // Prefer Communications microphone; fall back to default Console microphone.
        // SAFETY: Querying default Windows capture endpoint.
        unsafe { enumerator.GetDefaultAudioEndpoint(eCapture, eCommunications) }.or_else(
            |_: windows::core::Error| unsafe {
                enumerator.GetDefaultAudioEndpoint(eCapture, eConsole)
            },
        )
    }
}

#[cfg(windows)]
fn run_loopback_worker(
    stop_flag: &AtomicBool,
    stop_event_raw: usize,
    render_ring: &RenderReferenceRing,
) {
    let _com: ComInitGuard = ComInitGuard::init_mta();
    let stop_event: HANDLE = HANDLE(stop_event_raw as *mut c_void);

    while !stop_flag.load(Ordering::Acquire) {
        let loop_res: Result<(), windows::core::Error> =
            (|| -> Result<(), windows::core::Error> {
                // SAFETY: Creating standard Windows `MMDeviceEnumerator` COM object on MTA thread.
                let enumerator: IMMDeviceEnumerator =
                    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
                let device: IMMDevice = open_default_device(&enumerator, true)?;
                // SAFETY: Activating `IAudioClient` on default render endpoint.
                let audio_client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }?;
                // SAFETY: Retrieving shared-mode mix format owned by `CoTaskMemWaveFormatGuard`.
                let mix_ptr: *mut WAVEFORMATEX = unsafe { audio_client.GetMixFormat() }?;
                let _fmt_guard: CoTaskMemWaveFormatGuard =
                    CoTaskMemWaveFormatGuard { ptr: mix_ptr };
                // SAFETY: `mix_ptr` is non-null from successful `GetMixFormat`.
                let Some(stream_fmt): Option<WasapiStreamFormat> =
                    (unsafe { parse_wave_format(mix_ptr) })
                else {
                    log_line("wasapi loopback: unsupported mix format");
                    return Ok(());
                };

                // SAFETY: Shared-mode loopback capture requires `AUDCLNT_STREAMFLAGS_LOOPBACK`.
                unsafe {
                    audio_client.Initialize(
                        AUDCLNT_SHAREMODE_SHARED,
                        AUDCLNT_STREAMFLAGS_LOOPBACK,
                        WASAPI_BUFFER_DURATION_HNS,
                        0,
                        mix_ptr,
                        None,
                    )?;
                }
                // SAFETY: Retrieving `IAudioCaptureClient` service on initialized `IAudioClient`.
                let capture_client: IAudioCaptureClient = unsafe { audio_client.GetService() }?;
                // SAFETY: Starting loopback stream.
                unsafe {
                    audio_client.Start()?;
                }

                let mut resampler: LinearResampler =
                    LinearResampler::new(stream_fmt.sample_rate, VOICE_SAMPLE_RATE_HZ);
                let mut mono_raw: Vec<f32> = Vec::with_capacity(2048);
                let mut accum_48k: Vec<f32> = Vec::with_capacity(2048);
                let mut drain_cursor: usize = 0;

                while !stop_flag.load(Ordering::Acquire) {
                    // SAFETY: Interruptible 10 ms timer poll (loopback endpoints do not fire events on silence).
                    let wait_res: windows::Win32::Foundation::WAIT_EVENT =
                        unsafe { WaitForSingleObject(stop_event, WASAPI_LOOPBACK_POLL_MS) };
                    if wait_res == WAIT_OBJECT_0 || stop_flag.load(Ordering::Acquire) {
                        break;
                    }

                    let mut got_audio: bool = false;
                    loop {
                        // SAFETY: Querying pending frame count in loopback buffer.
                        let packet_frames: u32 = unsafe { capture_client.GetNextPacketSize() }?;
                        if packet_frames == 0 {
                            break;
                        }
                        let mut data_ptr: *mut u8 = std::ptr::null_mut();
                        let mut num_frames: u32 = 0;
                        let mut flags: u32 = 0;
                        // SAFETY: Acquiring readable loopback audio packet slice.
                        unsafe {
                            capture_client.GetBuffer(
                                &raw mut data_ptr,
                                &raw mut num_frames,
                                &raw mut flags,
                                None,
                                None,
                            )?;
                        }

                        let frames_usize: usize = usize::try_from(num_frames).unwrap_or(0);
                        mono_raw.clear();
                        let is_silent: bool =
                            (flags & u32::try_from(AUDCLNT_BUFFERFLAGS_SILENT.0).unwrap_or(2)) != 0;
                        if is_silent || data_ptr.is_null() {
                            mono_raw.resize(frames_usize, 0.0);
                        } else {
                            let byte_len: usize =
                                frames_usize.saturating_mul(usize::from(stream_fmt.block_align));
                            // SAFETY: `data_ptr` is valid for `byte_len` bytes until `ReleaseBuffer`.
                            let raw_slice: &[u8] =
                                unsafe { std::slice::from_raw_parts(data_ptr, byte_len) };
                            decode_interleaved_to_mono_f32(
                                raw_slice,
                                frames_usize,
                                stream_fmt,
                                &mut mono_raw,
                            );
                        }

                        // SAFETY: Always release acquired WASAPI buffer even if empty.
                        unsafe {
                            capture_client.ReleaseBuffer(num_frames)?;
                        }

                        if !mono_raw.is_empty() {
                            got_audio = true;
                            resampler.process(&mono_raw, &mut accum_48k);
                            while accum_48k.len().saturating_sub(drain_cursor)
                                >= DSP_FRAME_SAMPLES_10MS
                            {
                                let mut frame: [f32; DSP_FRAME_SAMPLES_10MS] =
                                    [0.0; DSP_FRAME_SAMPLES_10MS];
                                frame.copy_from_slice(
                                    &accum_48k[drain_cursor..drain_cursor + DSP_FRAME_SAMPLES_10MS],
                                );
                                render_ring.push_frame(frame);
                                drain_cursor += DSP_FRAME_SAMPLES_10MS;
                            }
                            if drain_cursor > 0 {
                                let _: std::vec::Drain<'_, f32> = accum_48k.drain(..drain_cursor);
                                drain_cursor = 0;
                            }
                            if accum_48k.len() > WASAPI_MAX_ACCUMULATOR_SAMPLES {
                                accum_48k.clear();
                            }
                        }
                    }

                    // Feed silence frame when game audio output is completely idle so AEC3 tracks silence cleanly.
                    if !got_audio {
                        render_ring.push_frame([0.0; DSP_FRAME_SAMPLES_10MS]);
                    }
                }

                // SAFETY: Stop loopback stream cleanly before dropping COM interfaces.
                let _stopped: Result<(), windows::core::Error> = unsafe { audio_client.Stop() };
                Ok(())
            })();

        if stop_flag.load(Ordering::Acquire) {
            break;
        }
        if let Err(err) = loop_res {
            let hr: i32 = err.code().0;
            if is_recoverable_device_error(hr) {
                log_line(&format!(
                    "wasapi loopback: endpoint invalidated (0x{:08X}), re-enumerating",
                    hr.cast_unsigned()
                ));
            } else {
                log_line(&format!(
                    "wasapi loopback: error 0x{:08X}, retrying",
                    hr.cast_unsigned()
                ));
            }
        }
        render_ring.clear();
        // SAFETY: Interruptible backoff wait before re-opening default render endpoint.
        let _w: windows::Win32::Foundation::WAIT_EVENT =
            unsafe { WaitForSingleObject(stop_event, WASAPI_RECONNECT_BACKOFF_MS) };
    }
}

#[cfg(windows)]
fn run_capture_worker(
    stop_flag: &AtomicBool,
    stop_event_raw: usize,
    render_ring: &RenderReferenceRing,
    make_handler: CaptureFrameFactory,
) {
    let _com: ComInitGuard = ComInitGuard::init_mta();
    let stop_event: HANDLE = HANDLE(stop_event_raw as *mut c_void);
    let mut on_frame: CaptureFrameHandler = make_handler();

    while !stop_flag.load(Ordering::Acquire) {
        let loop_res: Result<(), windows::core::Error> =
            (|| -> Result<(), windows::core::Error> {
                // SAFETY: Creating standard Windows `MMDeviceEnumerator` COM object on MTA thread.
                let enumerator: IMMDeviceEnumerator =
                    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
                let device: IMMDevice = open_default_device(&enumerator, false)?;
                // SAFETY: Activating `IAudioClient` on default capture microphone.
                let audio_client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }?;
                // SAFETY: Retrieving shared-mode mix format owned by `CoTaskMemWaveFormatGuard`.
                let mix_ptr: *mut WAVEFORMATEX = unsafe { audio_client.GetMixFormat() }?;
                let _fmt_guard: CoTaskMemWaveFormatGuard =
                    CoTaskMemWaveFormatGuard { ptr: mix_ptr };
                // SAFETY: `mix_ptr` is non-null from successful `GetMixFormat`.
                let Some(stream_fmt): Option<WasapiStreamFormat> =
                    (unsafe { parse_wave_format(mix_ptr) })
                else {
                    log_line("wasapi capture: unsupported mix format");
                    return Ok(());
                };

                // SAFETY: Creating auto-reset Win32 event for event-driven WASAPI capture callbacks.
                let cap_event_raw: HANDLE = unsafe { CreateEventW(None, false, false, None) }?;
                let cap_event: OwnedEventHandle = OwnedEventHandle(cap_event_raw);

                // SAFETY: Shared-mode event-driven capture on default microphone.
                unsafe {
                    audio_client.Initialize(
                        AUDCLNT_SHAREMODE_SHARED,
                        AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                        WASAPI_BUFFER_DURATION_HNS,
                        0,
                        mix_ptr,
                        None,
                    )?;
                    audio_client.SetEventHandle(cap_event.0)?;
                }
                // SAFETY: Retrieving `IAudioCaptureClient` service on initialized `IAudioClient`.
                let capture_client: IAudioCaptureClient = unsafe { audio_client.GetService() }?;
                // SAFETY: Starting microphone capture stream.
                unsafe {
                    audio_client.Start()?;
                }

                log_line(&format!(
                    "wasapi capture started ({} Hz, {} ch, {:?})",
                    stream_fmt.sample_rate, stream_fmt.channels, stream_fmt.sample_format
                ));

                let mut resampler: LinearResampler =
                    LinearResampler::new(stream_fmt.sample_rate, VOICE_SAMPLE_RATE_HZ);
                let mut mono_raw: Vec<f32> = Vec::with_capacity(2048);
                let mut accum_48k: Vec<f32> = Vec::with_capacity(2048);
                let mut drain_cursor: usize = 0;
                let wait_handles: [HANDLE; 2] = [stop_event, cap_event.0];

                while !stop_flag.load(Ordering::Acquire) {
                    // SAFETY: Waiting on `[stop_event, cap_event]` with bounded `50 ms` timeout.
                    let wait_res: windows::Win32::Foundation::WAIT_EVENT = unsafe {
                        WaitForMultipleObjects(&wait_handles, false, WASAPI_CAPTURE_WAIT_MS)
                    };
                    if wait_res == WAIT_OBJECT_0 || stop_flag.load(Ordering::Acquire) {
                        break;
                    }

                    loop {
                        // SAFETY: Querying pending captured frame count.
                        let packet_frames: u32 = unsafe { capture_client.GetNextPacketSize() }?;
                        if packet_frames == 0 {
                            break;
                        }
                        let mut data_ptr: *mut u8 = std::ptr::null_mut();
                        let mut num_frames: u32 = 0;
                        let mut flags: u32 = 0;
                        // SAFETY: Acquiring readable microphone capture packet slice.
                        unsafe {
                            capture_client.GetBuffer(
                                &raw mut data_ptr,
                                &raw mut num_frames,
                                &raw mut flags,
                                None,
                                None,
                            )?;
                        }

                        let frames_usize: usize = usize::try_from(num_frames).unwrap_or(0);
                        mono_raw.clear();
                        let is_silent: bool =
                            (flags & u32::try_from(AUDCLNT_BUFFERFLAGS_SILENT.0).unwrap_or(2)) != 0;
                        if is_silent || data_ptr.is_null() {
                            mono_raw.resize(frames_usize, 0.0);
                        } else {
                            let byte_len: usize =
                                frames_usize.saturating_mul(usize::from(stream_fmt.block_align));
                            // SAFETY: `data_ptr` is valid for `byte_len` bytes until `ReleaseBuffer`.
                            let raw_slice: &[u8] =
                                unsafe { std::slice::from_raw_parts(data_ptr, byte_len) };
                            decode_interleaved_to_mono_f32(
                                raw_slice,
                                frames_usize,
                                stream_fmt,
                                &mut mono_raw,
                            );
                        }

                        // SAFETY: Always release acquired WASAPI buffer.
                        unsafe {
                            capture_client.ReleaseBuffer(num_frames)?;
                        }

                        if !mono_raw.is_empty() {
                            resampler.process(&mono_raw, &mut accum_48k);
                            while accum_48k.len().saturating_sub(drain_cursor)
                                >= DSP_FRAME_SAMPLES_10MS
                            {
                                let mut cap_frame: [f32; DSP_FRAME_SAMPLES_10MS] =
                                    [0.0; DSP_FRAME_SAMPLES_10MS];
                                cap_frame.copy_from_slice(
                                    &accum_48k[drain_cursor..drain_cursor + DSP_FRAME_SAMPLES_10MS],
                                );
                                let render_opt: Option<[f32; DSP_FRAME_SAMPLES_10MS]> =
                                    render_ring.pop_frame();
                                on_frame(&cap_frame, render_opt.as_ref());
                                drain_cursor += DSP_FRAME_SAMPLES_10MS;
                            }
                            if drain_cursor > 0 {
                                let _: std::vec::Drain<'_, f32> = accum_48k.drain(..drain_cursor);
                                drain_cursor = 0;
                            }
                            if accum_48k.len() > WASAPI_MAX_ACCUMULATOR_SAMPLES {
                                accum_48k.clear();
                            }
                        }
                    }
                }

                // SAFETY: Stop capture stream cleanly before dropping COM interfaces.
                let _stopped: Result<(), windows::core::Error> = unsafe { audio_client.Stop() };
                Ok(())
            })();

        if stop_flag.load(Ordering::Acquire) {
            break;
        }
        if let Err(err) = loop_res {
            let hr: i32 = err.code().0;
            if is_recoverable_device_error(hr) {
                log_line(&format!(
                    "wasapi capture: microphone invalidated (0x{:08X}), hot-plug re-enumerating",
                    hr.cast_unsigned()
                ));
            } else {
                log_line(&format!(
                    "wasapi capture: error 0x{:08X}, retrying",
                    hr.cast_unsigned()
                ));
            }
        }
        // SAFETY: Interruptible backoff wait before re-opening microphone.
        let _w: windows::Win32::Foundation::WAIT_EVENT =
            unsafe { WaitForSingleObject(stop_event, WASAPI_RECONNECT_BACKOFF_MS) };
    }
}

impl WasapiSession {
    /// Starts both the WASAPI loopback render-reference thread and the event-driven WASAPI
    /// microphone capture thread, invoking the thread-local handler built by `make_handler`
    /// for every `10 ms` (`480`-sample @ `48 kHz`) captured frame.
    #[cfg(windows)]
    #[must_use]
    pub fn start(make_handler: CaptureFrameFactory) -> Option<Self> {
        // SAFETY: Creating manual-reset Win32 stop event shared across both worker threads.
        let stop_event: HANDLE = unsafe { CreateEventW(None, true, false, None) }.ok()?;
        let stop_event_raw: usize = stop_event.0 as usize;
        let stop_flag: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let render_ring: Arc<RenderReferenceRing> =
            Arc::new(RenderReferenceRing::new(WASAPI_RENDER_RING_CAPACITY_FRAMES));

        let lb_flag: Arc<AtomicBool> = Arc::clone(&stop_flag);
        let lb_ring: Arc<RenderReferenceRing> = Arc::clone(&render_ring);
        let loopback_thread: Option<thread::JoinHandle<()>> = thread::Builder::new()
            .name("dh-wasapi-loopback".to_string())
            .spawn(move || run_loopback_worker(&lb_flag, stop_event_raw, &lb_ring))
            .ok();

        let cap_flag: Arc<AtomicBool> = Arc::clone(&stop_flag);
        let capture_thread: Option<thread::JoinHandle<()>> = thread::Builder::new()
            .name("dh-wasapi-capture".to_string())
            .spawn(move || {
                run_capture_worker(&cap_flag, stop_event_raw, &render_ring, make_handler);
            })
            .ok();

        Some(Self {
            stop_flag,
            stop_event_raw,
            capture_thread,
            loopback_thread,
        })
    }

    /// Non-Windows stub.
    #[cfg(not(windows))]
    #[must_use]
    pub fn start(_make_handler: CaptureFrameFactory) -> Option<Self> {
        None
    }

    /// Signals both WASAPI worker threads to stop and joins them cleanly, releasing all
    /// microphone and loopback COM handles.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        #[cfg(windows)]
        if self.stop_event_raw != 0 {
            let stop_event: HANDLE = HANDLE(self.stop_event_raw as *mut c_void);
            // SAFETY: Signaling manual-reset event wakes both worker threads immediately.
            let _signaled: Result<(), windows::core::Error> = unsafe { SetEvent(stop_event) };
        }
        if let Some(handle) = self.capture_thread.take() {
            let _joined: std::thread::Result<()> = handle.join();
        }
        if let Some(handle) = self.loopback_thread.take() {
            let _joined: std::thread::Result<()> = handle.join();
        }
        #[cfg(windows)]
        if self.stop_event_raw != 0 {
            let stop_event: HANDLE = HANDLE(self.stop_event_raw as *mut c_void);
            self.stop_event_raw = 0;
            // SAFETY: Both worker threads have exited; safe to close the stop event handle.
            let _closed: Result<(), windows::core::Error> = unsafe { CloseHandle(stop_event) };
        }
    }
}

impl Drop for WasapiSession {
    fn drop(&mut self) {
        self.stop();
    }
}
