use std::ffi::c_void;

/// UE4 `FString` layout (`TArray<TCHAR>` where `TCHAR = u16` on Windows, `16` bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FString {
    /// Pointer to UTF-16LE NUL-terminated buffer.
    pub data: *mut u16,
    /// Element count including trailing NUL (`ArrayNum`).
    pub num: i32,
    /// Allocated element capacity (`ArrayMax`).
    pub max: i32,
}

/// UE4 `TArrayView<const int16>` layout (`16` bytes on x86-64: pointer + `i32` length + padding).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TArrayViewI16 {
    /// Pointer to contiguous signed 16-bit PCM audio samples.
    pub data: *const i16,
    /// Sample count (number of `i16` samples, NOT bytes).
    pub num: i32,
    /// Explicit struct tail padding matching MSVC x64 layout.
    pub pad: i32,
}

/// Calling convention for `FEngineLoop::Tick(FEngineLoop* this)`.
pub type EngineLoopTickFn = unsafe extern "C" fn(engine_loop: *mut c_void);

/// Calling convention for `FSocketBSD::RecvFrom(this, uint8*, i32, i32*, FInternetAddr*, u32) -> bool`.
pub type SocketRecvFromFn = unsafe extern "C" fn(
    socket: *mut c_void,
    data: *mut u8,
    buffer_size: i32,
    bytes_read: *mut i32,
    source: *mut c_void,
    flags: u32,
) -> u8;

/// Calling convention for `FSocketBSD::SendTo(this, const uint8*, i32, i32*, const FInternetAddr*) -> bool`.
pub type SocketSendToFn = unsafe extern "C" fn(
    socket: *mut c_void,
    data: *const u8,
    count: i32,
    bytes_sent: *mut i32,
    dest: *const c_void,
) -> u8;

/// Calling convention for `FSocketBSD::Close(FSocketBSD* this) -> bool`.
pub type SocketCloseFn = unsafe extern "C" fn(socket: *mut c_void) -> u8;

/// Calling convention for `UIpNetDriver::GetSocket(UIpNetDriver* this) -> FSocket*`.
pub type GetNetDriverSocketFn = unsafe extern "C" fn(net_driver: *mut c_void) -> *mut c_void;

/// Calling convention for `UGameEngine::GetGameWorld(UGameEngine* this) -> UWorld*`.
pub type GetGameWorldFn = unsafe extern "C" fn(engine: *mut c_void) -> *mut c_void;

/// Calling convention for `UGameInstance::GetFirstLocalPlayerController(this, UWorld*)`.
pub type GetFirstLocalPcFn =
    unsafe extern "C" fn(game_instance: *mut c_void, world: *mut c_void) -> *mut c_void;

/// Calling convention for UE4 `UClass::GetPrivateStaticClass() -> UClass*`.
pub type GetPrivateStaticClassFn = unsafe extern "C" fn() -> *mut c_void;

/// Calling convention for `FMemory::Free(void* original)`.
pub type FMemoryFreeFn = unsafe extern "C" fn(original: *mut c_void);

/// Calling convention for `UDH_OnlineVoice::BeforeRecvAudioRendered`.
///
/// NOTE: `online_voice_slot` is `UDH_OnlineVoice**` (pointer-to-pointer), verified at `0xE9F9DD`.
pub type BeforeRecvAudioRenderedFn = unsafe extern "C" fn(
    online_voice_slot: *const *mut c_void,
    sender_id_str: *const FString,
    pcm_samples: *const TArrayViewI16,
);

/// Calling convention for `UDH_OnlineVoice::GetVoiceChatUser(this) -> IVoiceChatUser*`.
pub type GetVoiceChatUserFn = unsafe extern "C" fn(online_voice: *mut c_void) -> *mut c_void;

/// Calling convention for `UDH_OnlineVoice::SetWantsToTalk(this, bool bWantsToTalk)`.
pub type SetWantsToTalkFn = unsafe extern "C" fn(online_voice: *mut c_void, wants_to_talk: u8);

/// Calling convention for `UDH_OnlineVoice::GetWantsToTalk(this) -> bool`.
pub type GetWantsToTalkFn = unsafe extern "C" fn(online_voice: *mut c_void) -> u8;

/// Calling convention for `UDH_OnlineVoice::IsParticipantSpeaking(this, ADH_PlayerState*, bool) -> bool`.
pub type IsParticipantSpeakingFn = unsafe extern "C" fn(
    online_voice: *mut c_void,
    player_state: *mut c_void,
    check_range: u8,
) -> u8;

/// Calling convention for `UDH_OnlineVoice::IsInVoipRange(this, ADH_PlayerState*, float) -> bool`.
pub type IsInVoipRangeFn = unsafe extern "C" fn(
    online_voice: *mut c_void,
    player_state: *mut c_void,
    max_range: f32,
) -> u8;

/// Calling convention for `UDH_OnlineVoice::IsPlayerInVoiceChat(const FString*, UObject*) -> bool`.
///
/// NOTE: Parameter order is `(PlayerIdStr, WorldContextObject)`, verified in `CanMuteParticipant`.
pub type IsPlayerInVoiceChatFn =
    unsafe extern "C" fn(player_id_str: *const FString, world_ctx: *mut c_void) -> u8;

/// Calling convention for `UDH_GameplayStatics::GetDHPlayerStateFromStringUniqueId(const FString*, UObject*)`.
pub type GetDhPlayerStateFromUniqueIdFn =
    unsafe extern "C" fn(unique_id_str: *const FString, world_ctx: *mut c_void) -> *mut c_void;

/// Calling convention for `UDH_GameplayStatics::GetLocalDHPlayerState(UObject*) -> ADH_PlayerState*`.
pub type GetLocalDhPlayerStateFn = unsafe extern "C" fn(world_ctx: *mut c_void) -> *mut c_void;

/// Calling convention for `FUniqueNetId::IsValid(this) -> bool`.
pub type UniqueNetIdIsValidFn = unsafe extern "C" fn(unique_id: *const c_void) -> u8;

/// Calling convention for `FUniqueNetId::ToString(this, out FString*) -> FString*`.
pub type UniqueNetIdToStringFn =
    unsafe extern "C" fn(unique_id: *const c_void, out_str: *mut FString) -> *mut FString;

/// Calling convention for `IVoiceChatUser::SetAudioInputVolume(this, float Volume)`.
pub type SetAudioInputVolumeFn = unsafe extern "C" fn(voice_chat_user: *mut c_void, volume: f32);

/// Calling convention for `IVoiceChatUser::SetAudio{Input,Output}DeviceMuted(this, bool bMuted)`.
pub type SetAudioMutedFn = unsafe extern "C" fn(voice_chat_user: *mut c_void, muted: u8);

/// Calling convention for `IVoiceChatUser::GetAudio{Input,Output}DeviceMuted(this) -> bool`.
pub type GetAudioMutedFn = unsafe extern "C" fn(voice_chat_user: *mut c_void) -> u8;
