pub mod dsp;
pub mod engine;
pub mod jitter;
pub mod opus;
pub mod patch;
pub mod proxy;
pub mod state;
pub mod ue4;
pub mod wasapi;

pub use dsp::{AgcLimiterState, BiquadHpfState, DspFrameResult, DspPipeline, VadGateState};
pub use engine::{
    ClientVoiceSession, DecodedPlayoutFrame, GameSocketEndpoint, RemotePeerVoice,
    TxCapturePipeline, VoiceRxState,
};
pub use jitter::{BufferedVoicePacket, JitterStats, PlayoutAction, SpeakerJitterBuffer};
pub use opus::{
    OpusApiTable, OpusDecodeFn, OpusDecoder, OpusDecoderCtlVoidFn, OpusDecoderGetSizeFn,
    OpusDecoderInitFn, OpusEncodeFn, OpusEncoder, OpusEncoderCtlSetIntFn, OpusEncoderCtlVoidFn,
    OpusEncoderGetSizeFn, OpusEncoderInitFn, OpusError, OpusGetVersionStringFn,
};
pub use patch::{
    DetourSpec, EosRecvGuardSpec, PatchOutcome, PatchReport, PendingSitePatch,
    StandardDetourBinding, TrampolineError, TrampolinePageBuilder,
};
pub use proxy::ProxyExportSpec;
pub use state::{MatchContext, SpeakerEntry};
pub use ue4::{
    BeforeRecvAudioRenderedFn, EngineLoopTickFn, FMemoryFreeFn, FString, GetAudioMutedFn,
    GetDhPlayerStateFromUniqueIdFn, GetFirstLocalPcFn, GetGameWorldFn, GetLocalDhPlayerStateFn,
    GetNetDriverSocketFn, GetParticipantAudioEnergyFn, GetPrivateStaticClassFn, GetVoiceChatUserFn,
    GetWantsToTalkFn, IsInVoipRangeFn, IsParticipantSpeakingFn, IsPlayerInVoiceChatFn,
    ProcessMulticastDelegateFn, SetAudioInputVolumeFn, SetAudioMutedFn, SetWantsToTalkFn,
    SocketCloseFn, SocketRecvFromFn, SocketSendToFn, TArrayViewI16, UniqueNetIdIsValidFn,
    UniqueNetIdToStringFn,
};
pub use wasapi::{
    CaptureFrameFactory, CaptureFrameHandler, LinearResampler, RenderReferenceRing,
    WasapiSampleFormat, WasapiSession, WasapiStreamFormat,
};
