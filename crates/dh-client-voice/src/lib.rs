pub mod consts;
pub mod dsp;
pub mod engine;
pub mod hooks;
pub mod jitter;
pub mod logger;
pub mod opus_ffi;
pub mod patcher;
pub mod proxy;
pub mod structs;
pub mod wasapi;

pub use dsp::{
    decay_meter_energy, normalized_f32_to_i16, pcm_i16_rms_energy, rms_to_meter_energy,
    smooth_meter_energy, soft_knee_limit,
};
pub use engine::{
    apply_plc_fade_in_place, encode_uid_utf16_nul, parse_sockaddr_storage, read_local_mic_energy,
};
pub use hooks::collect_speaker_ui_notifications;
pub use jitter::{advance_playout_accumulator, plc_fade_gain, seq_diff};
pub use patcher::{
    align_down, align_up, apply_client_patches_if_target, encode_abs_jmp_stub,
    encode_eos_recv_guard_stub, encode_jmp_rel32, encode_original_trampoline,
    is_target_process_path, parse_pe_size_of_image,
};
pub use proxy::last_patch_report;
pub use structs::{
    AgcLimiterState, BiquadHpfState, BufferedVoicePacket, DecodedPlayoutFrame, DetourSpec,
    DspFrameResult, DspPipeline, EosRecvGuardSpec, FString, GameSocketEndpoint, JitterStats,
    LinearResampler, MatchContext, OpusApiTable, OpusDecoder, OpusEncoder, OpusError, PatchOutcome,
    PatchReport, PlayoutAction, ProxyExportSpec, RenderReferenceRing, SpeakerEntry,
    SpeakerJitterBuffer, TArrayViewI16, TrampolineError, VadGateState, VoiceRxState,
    WasapiSampleFormat, WasapiSession, WasapiStreamFormat,
};
pub use wasapi::{decode_interleaved_to_mono_f32, is_recoverable_device_error};
