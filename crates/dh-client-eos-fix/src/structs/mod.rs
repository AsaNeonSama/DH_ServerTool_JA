pub mod patch;
pub mod proxy;

pub use patch::{
    AddLobbyAttributePrivateUriSpec, BytePatch, OnDestroyClearStartedSpec, OnJoinResetUriSpec,
    OnParticipantsKeepMembersSpec, OwnerResetTrampolineSpec, PatchOutcome, PatchReport,
    PostUpdateUriResetSpec, SetLobbyAttributesMaxLenSpec, SetUriStartTimeTrampolineSpec,
    TrampolineError,
};
pub use proxy::ProxyExportSpec;
