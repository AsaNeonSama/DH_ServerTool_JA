use std::ffi::{c_char, c_void, CStr};

use crate::consts::dsp::OPUS_FRAME_SAMPLES_20MS;
use crate::consts::opus::{
    OPUS_APPLICATION_VOIP, OPUS_MAX_STATE_BYTES, OPUS_OK, OPUS_RESET_STATE_REQUEST,
    OPUS_SET_BITRATE_REQUEST, OPUS_SET_COMPLEXITY_REQUEST, OPUS_SET_DTX_REQUEST,
    OPUS_SET_INBAND_FEC_REQUEST, OPUS_SET_PACKET_LOSS_PERC_REQUEST, OPUS_SET_SIGNAL_REQUEST,
    OPUS_SIGNAL_VOICE, OPUS_VOICE_BITRATE_BPS, OPUS_VOICE_CHANNELS, OPUS_VOICE_COMPLEXITY,
    OPUS_VOICE_EXPECTED_LOSS_PERC, OPUS_VOICE_SAMPLE_RATE, RVA_OPUS_DECODE, RVA_OPUS_DECODER_CTL,
    RVA_OPUS_DECODER_GET_SIZE, RVA_OPUS_DECODER_INIT, RVA_OPUS_ENCODE, RVA_OPUS_ENCODER_CTL,
    RVA_OPUS_ENCODER_GET_SIZE, RVA_OPUS_ENCODER_INIT, RVA_OPUS_GET_VERSION_STRING, SIG_OPUS_DECODE,
    SIG_OPUS_DECODER_CTL, SIG_OPUS_DECODER_GET_SIZE, SIG_OPUS_DECODER_INIT, SIG_OPUS_ENCODE,
    SIG_OPUS_ENCODER_CTL, SIG_OPUS_ENCODER_GET_SIZE, SIG_OPUS_ENCODER_INIT,
    SIG_OPUS_GET_VERSION_STRING,
};
pub use crate::structs::opus::{
    OpusApiTable, OpusDecodeFn, OpusDecoder, OpusDecoderCtlVoidFn, OpusDecoderGetSizeFn,
    OpusDecoderInitFn, OpusEncodeFn, OpusEncoder, OpusEncoderCtlSetIntFn, OpusEncoderCtlVoidFn,
    OpusEncoderGetSizeFn, OpusEncoderInitFn, OpusError, OpusGetVersionStringFn,
};

fn verify_signature_in_image(
    module_base: usize,
    image_size: usize,
    rva: usize,
    expected: &[u8; 8],
) -> Option<usize> {
    let end_rva: usize = rva.checked_add(expected.len())?;
    if end_rva > image_size {
        return None;
    }
    let addr: usize = module_base.checked_add(rva)?;
    // SAFETY: `addr..addr + 8` is bounds-checked inside the loaded client executable image.
    let actual: &[u8] = unsafe { std::slice::from_raw_parts(addr as *const u8, expected.len()) };
    if actual == expected {
        Some(addr)
    } else {
        None
    }
}

impl OpusApiTable {
    /// Resolves and signature-verifies all 9 in-process `libopus` entry points from the
    /// loaded `DreadHunger-Win64-Shipping.exe` image.
    ///
    /// # Safety
    /// `module_base` and `image_size` must describe the live mapped `DreadHunger-Win64-Shipping.exe`
    /// module in the current process.
    pub unsafe fn resolve_from_client_image(
        module_base: usize,
        image_size: usize,
    ) -> Result<Self, OpusError> {
        let enc_get_size_addr: usize = verify_signature_in_image(
            module_base,
            image_size,
            RVA_OPUS_ENCODER_GET_SIZE,
            &SIG_OPUS_ENCODER_GET_SIZE,
        )
        .ok_or(OpusError::SignatureMismatch)?;
        let enc_init_addr: usize = verify_signature_in_image(
            module_base,
            image_size,
            RVA_OPUS_ENCODER_INIT,
            &SIG_OPUS_ENCODER_INIT,
        )
        .ok_or(OpusError::SignatureMismatch)?;
        let enc_ctl_addr: usize = verify_signature_in_image(
            module_base,
            image_size,
            RVA_OPUS_ENCODER_CTL,
            &SIG_OPUS_ENCODER_CTL,
        )
        .ok_or(OpusError::SignatureMismatch)?;
        let enc_addr: usize =
            verify_signature_in_image(module_base, image_size, RVA_OPUS_ENCODE, &SIG_OPUS_ENCODE)
                .ok_or(OpusError::SignatureMismatch)?;
        let dec_get_size_addr: usize = verify_signature_in_image(
            module_base,
            image_size,
            RVA_OPUS_DECODER_GET_SIZE,
            &SIG_OPUS_DECODER_GET_SIZE,
        )
        .ok_or(OpusError::SignatureMismatch)?;
        let dec_init_addr: usize = verify_signature_in_image(
            module_base,
            image_size,
            RVA_OPUS_DECODER_INIT,
            &SIG_OPUS_DECODER_INIT,
        )
        .ok_or(OpusError::SignatureMismatch)?;
        let dec_ctl_addr: usize = verify_signature_in_image(
            module_base,
            image_size,
            RVA_OPUS_DECODER_CTL,
            &SIG_OPUS_DECODER_CTL,
        )
        .ok_or(OpusError::SignatureMismatch)?;
        let dec_addr: usize =
            verify_signature_in_image(module_base, image_size, RVA_OPUS_DECODE, &SIG_OPUS_DECODE)
                .ok_or(OpusError::SignatureMismatch)?;
        let ver_addr: usize = verify_signature_in_image(
            module_base,
            image_size,
            RVA_OPUS_GET_VERSION_STRING,
            &SIG_OPUS_GET_VERSION_STRING,
        )
        .ok_or(OpusError::SignatureMismatch)?;

        // SAFETY: All 9 addresses have been bounds-checked and prologue-verified against
        // `DreadHunger-Win64-Shipping.exe`'s embedded `libopus 1.1-beta` Win64 C ABI functions.
        unsafe {
            Ok(Self {
                encoder_get_size: std::mem::transmute::<usize, OpusEncoderGetSizeFn>(
                    enc_get_size_addr,
                ),
                encoder_init: std::mem::transmute::<usize, OpusEncoderInitFn>(enc_init_addr),
                encoder_ctl_set_int: std::mem::transmute::<usize, OpusEncoderCtlSetIntFn>(
                    enc_ctl_addr,
                ),
                encoder_ctl_void: std::mem::transmute::<usize, OpusEncoderCtlVoidFn>(enc_ctl_addr),
                encode: std::mem::transmute::<usize, OpusEncodeFn>(enc_addr),
                decoder_get_size: std::mem::transmute::<usize, OpusDecoderGetSizeFn>(
                    dec_get_size_addr,
                ),
                decoder_init: std::mem::transmute::<usize, OpusDecoderInitFn>(dec_init_addr),
                decoder_ctl_void: std::mem::transmute::<usize, OpusDecoderCtlVoidFn>(dec_ctl_addr),
                decode: std::mem::transmute::<usize, OpusDecodeFn>(dec_addr),
                get_version_string: std::mem::transmute::<usize, OpusGetVersionStringFn>(ver_addr),
            })
        }
    }

    /// Queries the static version banner string reported by the in-process `libopus` library.
    #[must_use]
    pub fn version_string(&self) -> String {
        // SAFETY: `opus_get_version_string` returns a static NUL-terminated ASCII string pointer.
        let ptr: *const c_char = unsafe { (self.get_version_string)() };
        if ptr.is_null() {
            return String::new();
        }
        // SAFETY: `ptr` is a valid non-null static C string inside `.rdata`.
        let cstr: &CStr = unsafe { CStr::from_ptr(ptr) };
        cstr.to_string_lossy().into_owned()
    }
}

fn alloc_aligned_state_buffer(raw_size: i32) -> Result<Vec<u128>, OpusError> {
    let size_usize: usize = usize::try_from(raw_size)
        .map_err(|_: std::num::TryFromIntError| OpusError::InvalidStateSize(raw_size))?;
    if size_usize == 0 || size_usize > OPUS_MAX_STATE_BYTES {
        return Err(OpusError::InvalidStateSize(raw_size));
    }
    let words: usize = size_usize.div_ceil(std::mem::size_of::<u128>());
    Ok(vec![0u128; words])
}

// SAFETY: `OpusEncoder` owns its `libopus` state buffer exclusively (`Vec<u128>`) with no thread-local state.
unsafe impl Send for OpusEncoder {}

impl OpusEncoder {
    /// Allocates and configures a `48 kHz` mono VOIP Opus encoder (`32 kbps`, In-Band FEC,
    /// `15%` expected packet loss, DTX, `OPUS_SIGNAL_VOICE`, complexity `5`) using Rust-owned
    /// 16-byte-aligned storage.
    pub fn new_voice_48k_mono(api: OpusApiTable) -> Result<Self, OpusError> {
        // SAFETY: Pure size query with valid channel count `1`.
        let raw_size: i32 = unsafe { (api.encoder_get_size)(OPUS_VOICE_CHANNELS) };
        let mut storage: Vec<u128> = alloc_aligned_state_buffer(raw_size)?;
        let state_ptr: *mut c_void = storage.as_mut_ptr().cast::<c_void>();

        // SAFETY: `state_ptr` is 16-byte aligned with at least `raw_size` writable bytes.
        let init_rc: i32 = unsafe {
            (api.encoder_init)(
                state_ptr,
                OPUS_VOICE_SAMPLE_RATE,
                OPUS_VOICE_CHANNELS,
                OPUS_APPLICATION_VOIP,
            )
        };
        if init_rc != OPUS_OK {
            return Err(OpusError::LibOpusCode(init_rc));
        }

        let ctl_pairs: [(i32, i32); 6] = [
            (OPUS_SET_BITRATE_REQUEST, OPUS_VOICE_BITRATE_BPS),
            (OPUS_SET_INBAND_FEC_REQUEST, 1),
            (
                OPUS_SET_PACKET_LOSS_PERC_REQUEST,
                OPUS_VOICE_EXPECTED_LOSS_PERC,
            ),
            (OPUS_SET_DTX_REQUEST, 1),
            (OPUS_SET_SIGNAL_REQUEST, OPUS_SIGNAL_VOICE),
            (OPUS_SET_COMPLEXITY_REQUEST, OPUS_VOICE_COMPLEXITY),
        ];
        for (req, val) in ctl_pairs {
            // SAFETY: `state_ptr` is an initialized `OpusEncoder`; `opus_encoder_ctl` spills
            // `r8` to `[rsp+0x18]` under Win64 x64 ABI for single-integer SET CTLs.
            let rc: i32 = unsafe { (api.encoder_ctl_set_int)(state_ptr, req, val) };
            if rc != OPUS_OK {
                return Err(OpusError::LibOpusCode(rc));
            }
        }

        Ok(Self { api, storage })
    }

    /// Resets internal encoder state cleanly between matches or talkspurt epochs.
    pub fn reset(&mut self) -> Result<(), OpusError> {
        let state_ptr: *mut c_void = self.storage.as_mut_ptr().cast::<c_void>();
        // SAFETY: `state_ptr` is a live initialized `OpusEncoder`.
        let rc: i32 = unsafe { (self.api.encoder_ctl_void)(state_ptr, OPUS_RESET_STATE_REQUEST) };
        if rc == OPUS_OK {
            Ok(())
        } else {
            Err(OpusError::LibOpusCode(rc))
        }
    }

    /// Encodes one `20 ms` (`960`-sample @ `48 kHz` mono) `i16` PCM frame into `out_packet`.
    pub fn encode_20ms(
        &mut self,
        pcm: &[i16; OPUS_FRAME_SAMPLES_20MS],
        out_packet: &mut [u8],
    ) -> Result<usize, OpusError> {
        if out_packet.is_empty() {
            return Err(OpusError::BufferTooSmall);
        }
        let max_bytes: i32 = i32::try_from(out_packet.len())
            .map_err(|_: std::num::TryFromIntError| OpusError::BufferTooSmall)?;
        let frame_samples: i32 = i32::try_from(OPUS_FRAME_SAMPLES_20MS).unwrap_or(960);
        let state_ptr: *mut c_void = self.storage.as_mut_ptr().cast::<c_void>();

        // SAFETY: `state_ptr` is initialized, `pcm` holds `960` readable `i16` samples, and
        // `out_packet` holds `max_bytes` writable bytes.
        let written: i32 = unsafe {
            (self.api.encode)(
                state_ptr,
                pcm.as_ptr(),
                frame_samples,
                out_packet.as_mut_ptr(),
                max_bytes,
            )
        };
        if written < 0 {
            return Err(OpusError::LibOpusCode(written));
        }
        usize::try_from(written).map_err(|_: std::num::TryFromIntError| OpusError::BufferTooSmall)
    }
}

// SAFETY: `OpusDecoder` owns its `libopus` state buffer exclusively (`Vec<u128>`) with no thread-local state.
unsafe impl Send for OpusDecoder {}

impl OpusDecoder {
    /// Allocates and initializes a `48 kHz` mono Opus decoder in Rust-owned 16-byte-aligned storage.
    pub fn new_voice_48k_mono(api: OpusApiTable) -> Result<Self, OpusError> {
        // SAFETY: Pure size query with valid channel count `1`.
        let raw_size: i32 = unsafe { (api.decoder_get_size)(OPUS_VOICE_CHANNELS) };
        let mut storage: Vec<u128> = alloc_aligned_state_buffer(raw_size)?;
        let state_ptr: *mut c_void = storage.as_mut_ptr().cast::<c_void>();

        // SAFETY: `state_ptr` is 16-byte aligned with at least `raw_size` writable bytes.
        let init_rc: i32 =
            unsafe { (api.decoder_init)(state_ptr, OPUS_VOICE_SAMPLE_RATE, OPUS_VOICE_CHANNELS) };
        if init_rc != OPUS_OK {
            return Err(OpusError::LibOpusCode(init_rc));
        }
        Ok(Self { api, storage })
    }

    /// Resets internal decoder state cleanly.
    pub fn reset(&mut self) -> Result<(), OpusError> {
        let state_ptr: *mut c_void = self.storage.as_mut_ptr().cast::<c_void>();
        // SAFETY: `state_ptr` is a live initialized `OpusDecoder`.
        let rc: i32 = unsafe { (self.api.decoder_ctl_void)(state_ptr, OPUS_RESET_STATE_REQUEST) };
        if rc == OPUS_OK {
            Ok(())
        } else {
            Err(OpusError::LibOpusCode(rc))
        }
    }

    /// Decodes a `20 ms` (`960`-sample @ `48 kHz` mono) frame:
    /// - `packet = Some(bytes), decode_fec = false`: normal Opus frame decode.
    /// - `packet = Some(next_bytes), decode_fec = true`: 1-packet In-Band FEC recovery.
    /// - `packet = None, decode_fec = false`: Opus Packet Loss Concealment (PLC).
    pub fn decode_20ms(
        &mut self,
        packet: Option<&[u8]>,
        out_pcm: &mut [i16; OPUS_FRAME_SAMPLES_20MS],
        decode_fec: bool,
    ) -> Result<usize, OpusError> {
        let (data_ptr, data_len): (*const u8, i32) = match packet {
            Some(bytes) => {
                let len_i32: i32 = i32::try_from(bytes.len())
                    .map_err(|_: std::num::TryFromIntError| OpusError::BufferTooSmall)?;
                (bytes.as_ptr(), len_i32)
            }
            None => (std::ptr::null(), 0),
        };
        let frame_samples: i32 = i32::try_from(OPUS_FRAME_SAMPLES_20MS).unwrap_or(960);
        let fec_flag: i32 = i32::from(decode_fec);
        let state_ptr: *mut c_void = self.storage.as_mut_ptr().cast::<c_void>();

        // SAFETY: `state_ptr` is initialized, `data_ptr` is either null (with `data_len == 0`) or
        // points to `data_len` readable bytes, and `out_pcm` holds `960` writable `i16` samples.
        let decoded: i32 = unsafe {
            (self.api.decode)(
                state_ptr,
                data_ptr,
                data_len,
                out_pcm.as_mut_ptr(),
                frame_samples,
                fec_flag,
            )
        };
        if decoded < 0 {
            return Err(OpusError::LibOpusCode(decoded));
        }
        usize::try_from(decoded).map_err(|_: std::num::TryFromIntError| OpusError::BufferTooSmall)
    }
}
