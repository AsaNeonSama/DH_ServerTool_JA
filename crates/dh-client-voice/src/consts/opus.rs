/// Verified Client RVAs of in-process `libopus 1.1-beta` functions inside
/// `DreadHunger-Win64-Shipping.exe` (Win64 `extern "C"` ABI).
pub const RVA_OPUS_ENCODER_GET_SIZE: usize = 0x0055_4F70;
pub const RVA_OPUS_ENCODER_INIT: usize = 0x0055_4FC0;
pub const RVA_OPUS_ENCODER_CTL: usize = 0x0055_48A0;
pub const RVA_OPUS_ENCODE: usize = 0x0055_2230;
pub const RVA_OPUS_DECODER_GET_SIZE: usize = 0x0055_8310;
pub const RVA_OPUS_DECODER_INIT: usize = 0x0055_8360;
pub const RVA_OPUS_DECODER_CTL: usize = 0x0055_8100;
pub const RVA_OPUS_DECODE: usize = 0x0055_6EB0;
pub const RVA_OPUS_GET_VERSION_STRING: usize = 0x0055_0510;

/// Expected leading machine-code bytes for runtime signature verification of in-process `libopus`.
pub const SIG_OPUS_ENCODER_GET_SIZE: [u8; 8] = [0x40, 0x53, 0x48, 0x83, 0xEC, 0x20, 0x8D, 0x41];
pub const SIG_OPUS_ENCODER_INIT: [u8; 8] = [0x40, 0x53, 0x56, 0x57, 0x41, 0x56, 0x48, 0x83];
pub const SIG_OPUS_ENCODER_CTL: [u8; 8] = [0x89, 0x54, 0x24, 0x10, 0x4C, 0x89, 0x44, 0x24];
pub const SIG_OPUS_ENCODE: [u8; 8] = [0x40, 0x55, 0x41, 0x56, 0x41, 0x57, 0x48, 0x83];
pub const SIG_OPUS_DECODER_GET_SIZE: [u8; 8] = [0x40, 0x53, 0x48, 0x83, 0xEC, 0x20, 0x8D, 0x41];
pub const SIG_OPUS_DECODER_INIT: [u8; 8] = [0x40, 0x53, 0x56, 0x57, 0x48, 0x83, 0xEC, 0x20];
pub const SIG_OPUS_DECODER_CTL: [u8; 8] = [0x89, 0x54, 0x24, 0x10, 0x4C, 0x89, 0x44, 0x24];
pub const SIG_OPUS_DECODE: [u8; 8] = [0x40, 0x55, 0x48, 0x83, 0xEC, 0x60, 0x48, 0x8D];
pub const SIG_OPUS_GET_VERSION_STRING: [u8; 8] = [0x48, 0x8D, 0x05, 0xC1, 0xB3, 0xF6, 0x03, 0xC3];

/// Standard `libopus` return code indicating success (`OPUS_OK`).
pub const OPUS_OK: i32 = 0;

/// Opus VOIP application mode (`OPUS_APPLICATION_VOIP`).
pub const OPUS_APPLICATION_VOIP: i32 = 2048;

/// Opus encoder/decoder CTL request IDs (standard `opus_defines.h`).
pub const OPUS_SET_BITRATE_REQUEST: i32 = 4002;
pub const OPUS_SET_COMPLEXITY_REQUEST: i32 = 4010;
pub const OPUS_SET_INBAND_FEC_REQUEST: i32 = 4012;
pub const OPUS_SET_PACKET_LOSS_PERC_REQUEST: i32 = 4014;
pub const OPUS_SET_DTX_REQUEST: i32 = 4016;
pub const OPUS_SET_SIGNAL_REQUEST: i32 = 4024;
pub const OPUS_RESET_STATE_REQUEST: i32 = 4028;

/// Opus voice signal hint (`OPUS_SIGNAL_VOICE`).
pub const OPUS_SIGNAL_VOICE: i32 = 3001;

/// Voice Opus encoder operating parameters (`48 kHz` mono VOIP, `32 kbps`, FEC + DTX).
pub const OPUS_VOICE_SAMPLE_RATE: i32 = 48_000;
pub const OPUS_VOICE_CHANNELS: i32 = 1;
pub const OPUS_VOICE_BITRATE_BPS: i32 = 32_000;
pub const OPUS_VOICE_COMPLEXITY: i32 = 5;
pub const OPUS_VOICE_EXPECTED_LOSS_PERC: i32 = 15;

/// Maximum encoded Opus packet byte length matching `dh-voice-proto` (`MAX_OPUS_PAYLOAD`).
pub const OPUS_MAX_PAYLOAD_BYTES: usize = 512;

/// Opus DTX comfort/silence frames are `<= 2` bytes; suppress TX on pure DTX silence frames.
pub const OPUS_DTX_SILENCE_MAX_BYTES: usize = 2;

/// Sanity cap on `opus_{encoder,decoder}_get_size` (`256 KiB`).
pub const OPUS_MAX_STATE_BYTES: usize = 262_144;
