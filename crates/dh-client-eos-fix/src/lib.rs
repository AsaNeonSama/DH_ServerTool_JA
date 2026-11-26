pub mod consts;
pub mod patcher;
pub mod proxy;
pub mod structs;

pub use patcher::{
    align_down, align_up, apply_client_patches_if_target, apply_direct_patch_in_buffer,
    encode_add_lobby_attribute_private_uri_trampoline, encode_call_rel32,
    encode_jmp_indirect_rel32, encode_jmp_rel32, encode_on_destroy_clear_started_trampoline,
    encode_on_join_reset_uri_trampoline, encode_on_participants_keep_members_trampoline,
    encode_owner_reset_trampoline, encode_post_update_empty_uri_trampoline,
    encode_set_lobby_attributes_max_len_trampoline, encode_set_uri_start_time_trampoline,
    is_target_process_path, parse_pe_size_of_image,
};
pub use proxy::last_patch_report;
pub use structs::{
    AddLobbyAttributePrivateUriSpec, BytePatch, OnDestroyClearStartedSpec, OnJoinResetUriSpec,
    OnParticipantsKeepMembersSpec, OwnerResetTrampolineSpec, PatchOutcome, PatchReport,
    PostUpdateUriResetSpec, ProxyExportSpec, SetLobbyAttributesMaxLenSpec,
    SetUriStartTimeTrampolineSpec, TrampolineError,
};
