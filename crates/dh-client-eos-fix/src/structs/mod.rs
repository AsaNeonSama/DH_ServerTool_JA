pub mod patch;
pub mod proxy;

pub use patch::{
    AddLobbyAttributePrivateUriSpec, BytePatch, OwnerResetTrampolineSpec, PatchOutcome,
    PatchReport, PostUpdateUriResetSpec, SetUriStartTimeTrampolineSpec, TrampolineError,
};
pub use proxy::ProxyExportSpec;
