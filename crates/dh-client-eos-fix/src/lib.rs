pub mod consts;
pub mod patcher;
pub mod proxy;
pub mod structs;

pub use patcher::{
    align_down, align_up, apply_client_patches_if_target, apply_direct_patch_in_buffer,
    encode_add_lobby_attribute_private_uri_trampoline, encode_call_rel32,
    encode_jmp_indirect_rel32, encode_owner_reset_trampoline,
    encode_post_update_empty_uri_trampoline, encode_set_uri_start_time_trampoline,
    is_target_process_path, parse_pe_size_of_image,
};
pub use proxy::last_patch_report;
pub use structs::{
    AddLobbyAttributePrivateUriSpec, BytePatch, OwnerResetTrampolineSpec, PatchOutcome,
    PatchReport, PostUpdateUriResetSpec, ProxyExportSpec, SetUriStartTimeTrampolineSpec,
    TrampolineError,
};
