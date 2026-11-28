use std::ffi::{c_char, c_void};

/// Calling conventions for in-process `libopus 1.1-beta` functions inside `DreadHunger-Win64-Shipping.exe`.
pub type OpusEncoderGetSizeFn = unsafe extern "C" fn(channels: i32) -> i32;
pub type OpusEncoderInitFn =
    unsafe extern "C" fn(state: *mut c_void, fs: i32, channels: i32, application: i32) -> i32;
/// Win64 variadic CTL setter (`opus_encoder_ctl` spills `r8` to `[rsp+0x18]`, safe to invoke
/// with a single `i32` argument on x86-64).
pub type OpusEncoderCtlSetIntFn =
    unsafe extern "C" fn(state: *mut c_void, request: i32, value: i32) -> i32;
pub type OpusEncoderCtlVoidFn = unsafe extern "C" fn(state: *mut c_void, request: i32) -> i32;
pub type OpusEncodeFn = unsafe extern "C" fn(
    state: *mut c_void,
    pcm: *const i16,
    frame_size: i32,
    data: *mut u8,
    max_data_bytes: i32,
) -> i32;

pub type OpusDecoderGetSizeFn = unsafe extern "C" fn(channels: i32) -> i32;
pub type OpusDecoderInitFn =
    unsafe extern "C" fn(state: *mut c_void, fs: i32, channels: i32) -> i32;
pub type OpusDecoderCtlVoidFn = unsafe extern "C" fn(state: *mut c_void, request: i32) -> i32;
pub type OpusDecodeFn = unsafe extern "C" fn(
    state: *mut c_void,
    data: *const u8,
    len: i32,
    pcm: *mut i16,
    frame_size: i32,
    decode_fec: i32,
) -> i32;
pub type OpusGetVersionStringFn = unsafe extern "C" fn() -> *const c_char;

/// Resolved function table pointing to verified in-process `libopus` entry points.
#[derive(Clone, Copy)]
pub struct OpusApiTable {
    pub encoder_get_size: OpusEncoderGetSizeFn,
    pub encoder_init: OpusEncoderInitFn,
    pub encoder_ctl_set_int: OpusEncoderCtlSetIntFn,
    pub encoder_ctl_void: OpusEncoderCtlVoidFn,
    pub encode: OpusEncodeFn,
    pub decoder_get_size: OpusDecoderGetSizeFn,
    pub decoder_init: OpusDecoderInitFn,
    pub decoder_ctl_void: OpusDecoderCtlVoidFn,
    pub decode: OpusDecodeFn,
    pub get_version_string: OpusGetVersionStringFn,
}

/// Error returned by the safe in-process `libopus` wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpusError {
    /// Expected `libopus` prologue signature bytes did not match the loaded executable image.
    SignatureMismatch,
    /// `opus_{encoder,decoder}_get_size` returned an invalid or out-of-range size.
    InvalidStateSize(i32),
    /// A `libopus` API call returned a negative error code.
    LibOpusCode(i32),
    /// Output buffer was too small or invalid.
    BufferTooSmall,
}

/// Safe Rust wrapper around an in-process `libopus` encoder backed by Rust-owned 16-byte-aligned
/// storage (zero cross-allocator heap mismatch with UE4).
pub struct OpusEncoder {
    pub(crate) api: OpusApiTable,
    pub(crate) storage: Vec<u128>,
}

/// Safe Rust wrapper around an in-process `libopus` decoder backed by Rust-owned 16-byte-aligned
/// storage (zero cross-allocator heap mismatch with UE4).
pub struct OpusDecoder {
    pub(crate) api: OpusApiTable,
    pub(crate) storage: Vec<u128>,
}
