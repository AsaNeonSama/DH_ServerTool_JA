pub mod consts;
pub mod hooks;
pub mod logger;
pub mod patcher;
pub mod proxy;
pub mod structs;

pub use patcher::{
    align_down, align_up, apply_client_patches_if_target, apply_direct_patch_in_buffer,
    encode_abs_jmp_stub, encode_eos_recv_guard_stub, encode_jmp_rel32, encode_original_trampoline,
    encode_submit_remote_voice_stub, is_target_process_path, parse_pe_size_of_image,
};
pub use proxy::last_patch_report;
pub use structs::{
    BytePatch, ChannelDefinition, DetourSpec, EosRecvGuardSpec, FName, FString, MatchContext,
    PatchOutcome, PatchReport, ProxyExportSpec, SharedPtr, SubmitRemoteVoiceSpec, TArrayRaw,
    TArrayViewI16, TrampolineError, VtablePatchSpec,
};
