pub mod patch;
pub mod proxy;
pub mod state;
pub mod ue4;

pub use patch::{
    BytePatch, DetourSpec, EosRecvGuardSpec, PatchOutcome, PatchReport, PendingSitePatch,
    StandardDetourBinding, SubmitRemoteVoiceSpec, TrampolineError, TrampolinePageBuilder,
    VtablePatchSpec,
};
pub use proxy::ProxyExportSpec;
pub use state::{MatchContext, SpeakerEntry};
pub use ue4::{
    BeforeRecvAudioRenderedFn, ChannelDefinition, FMemoryFreeFn, FMemoryReallocFn, FName,
    FNameCtorAnsiFn, FNameEntryIdFromValidENameFn, FString, GetAudioMutedFn,
    GetDhPlayerStateFromUniqueIdFn, GetFirstLocalPcFn, GetGameWorldFn, GetLocalDhPlayerStateFn,
    GetPrivateStaticClassFn, GetUniquePlayerIdFn, GetVoiceChatUserFn, GetVoiceInterfaceFn,
    GetWantsToTalkFn, IsInVoipRangeFn, IsParticipantSpeakingFn, IsPlayerInVoiceChatFn,
    LoadChannelDefinitionsFn, OnlineVoiceTickFn, RefCtrlDestroyObjFn, RefCtrlDtorFn,
    RegisterLocalTalkerFn, SetAudioInputVolumeFn, SetAudioMutedFn, SetWantsToTalkFn, SharedPtr,
    StartStopNetworkedVoiceFn, TArrayRaw, TArrayViewI16, UniqueNetIdIsValidFn,
    UniqueNetIdToStringFn,
};
