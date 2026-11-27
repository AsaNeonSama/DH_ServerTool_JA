use std::ffi::{c_char, c_void};

/// UE4 `FName` layout (`8` bytes in Shipping builds).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FName {
    /// `FNameEntryId` comparison index into `GNamePool`.
    pub comparison_index: u32,
    /// Optional instance suffix number (`0` for plain names).
    pub number: u32,
}

/// UE4 `FString` layout (`TArray<TCHAR>` where `TCHAR = u16` on Windows, `16` bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FString {
    /// Pointer to UE4 `FMemory`-allocated UTF-16LE NUL-terminated buffer.
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

/// Raw UE4 `TArray<T>` header (`16` bytes on x86-64).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TArrayRaw {
    /// Pointer to heap-allocated elements managed by UE4 `FMemory`.
    pub data: *mut c_void,
    /// Active element count (`ArrayNum`).
    pub num: i32,
    /// Allocated element capacity (`ArrayMax`).
    pub max: i32,
}

/// UE4 `TSharedPtr<T>` layout (`16` bytes: raw object pointer + reference controller pointer).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SharedPtr {
    /// Pointed-to object (`nullptr` if empty).
    pub object: *mut c_void,
    /// `SharedPointerInternals::FReferenceControllerBase*` (`nullptr` if empty).
    pub controller: *mut c_void,
}

/// UE4 `FChannelDefinition` layout (`0x28` bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ChannelDefinition {
    /// Channel identifier (e.g. `NAME_Voice`).
    pub channel_name: FName,
    /// Class path name (e.g. `"/Script/Engine.VoiceChannel"`).
    pub class_name: FName,
    /// Resolved `UClass*` populated by `UNetDriver::LoadChannelDefinitions`.
    pub channel_class: *mut c_void,
    /// Static channel index (`1` for `Voice`, `-1` for dynamic allocation).
    pub static_channel_index: i32,
    /// Tick channel immediately on creation.
    pub b_tick_on_create: u8,
    /// Server may open this channel.
    pub b_server_open: u8,
    /// Client may open this channel.
    pub b_client_open: u8,
    /// Open automatically on initial server connection.
    pub b_initial_server: u8,
    /// Open automatically on initial client connection.
    pub b_initial_client: u8,
    /// Explicit trailing struct padding to match UE4 `sizeof(FChannelDefinition) == 0x28`.
    pub pad: [u8; 7],
}

/// Calling convention for `UNetDriver::LoadChannelDefinitions(UNetDriver* this)`.
pub type LoadChannelDefinitionsFn = unsafe extern "C" fn(net_driver: *mut c_void);

/// Calling convention for `FOnlineVoiceImpl::Tick(FOnlineVoiceImpl* this, float DeltaTime)`.
pub type OnlineVoiceTickFn = unsafe extern "C" fn(voice_impl: *mut c_void, delta_time: f32);

/// Calling convention for `IOnlineSubsystem::GetVoiceInterface(this, out TSharedPtr*) -> TSharedPtr*`.
pub type GetVoiceInterfaceFn =
    unsafe extern "C" fn(subsystem: *mut c_void, out_ptr: *mut SharedPtr) -> *mut SharedPtr;

/// Calling convention for `UGameEngine::GetGameWorld(UGameEngine* this) -> UWorld*`.
pub type GetGameWorldFn = unsafe extern "C" fn(engine: *mut c_void) -> *mut c_void;

/// Calling convention for `UGameInstance::GetFirstLocalPlayerController(this, UWorld*)`.
pub type GetFirstLocalPcFn =
    unsafe extern "C" fn(game_instance: *mut c_void, world: *mut c_void) -> *mut c_void;

/// Calling convention for UE4 `UClass::GetPrivateStaticClass() -> UClass*`.
pub type GetPrivateStaticClassFn = unsafe extern "C" fn() -> *mut c_void;

/// Calling convention for `FNameEntryId::FromValidEName(FNameEntryId* out, uint32 ename)`.
pub type FNameEntryIdFromValidENameFn =
    unsafe extern "C" fn(out_id: *mut u32, ename: u32) -> *mut u32;

/// Calling convention for `FName::FName(FName* this, const ANSICHAR* name, EFindName find_type)`.
pub type FNameCtorAnsiFn =
    unsafe extern "C" fn(out_name: *mut FName, name: *const c_char, find_type: u32) -> *mut FName;

/// Calling convention for `FMemory::Realloc(void* original, SIZE_T count, uint32 alignment)`.
pub type FMemoryReallocFn =
    unsafe extern "C" fn(original: *mut c_void, count: usize, alignment: u32) -> *mut c_void;

/// Calling convention for `FMemory::Free(void* original)`.
pub type FMemoryFreeFn = unsafe extern "C" fn(original: *mut c_void);

/// Calling convention for `FOnlineVoiceImpl::RegisterLocalTalker(this, uint32 LocalUserNum) -> bool`.
pub type RegisterLocalTalkerFn =
    unsafe extern "C" fn(voice_impl: *mut c_void, local_user_num: u32) -> u8;

/// Calling convention for `FOnlineVoiceImpl::{Start,Stop}NetworkedVoice(this, uint8 LocalUserNum)`.
pub type StartStopNetworkedVoiceFn =
    unsafe extern "C" fn(voice_impl: *mut c_void, local_user_num: u8);

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

/// Calling convention for `IOnlineIdentity::GetUniquePlayerId(this, out TSharedPtr*, int32 LocalUserNum)`.
pub type GetUniquePlayerIdFn = unsafe extern "C" fn(
    identity: *mut c_void,
    out_ptr: *mut SharedPtr,
    local_user_num: i32,
) -> *mut SharedPtr;

/// Calling convention for `FUniqueNetId::IsValid(this) -> bool`.
pub type UniqueNetIdIsValidFn = unsafe extern "C" fn(unique_id: *const c_void) -> u8;

/// Calling convention for `FUniqueNetId::ToString(this, out FString*) -> FString*`.
pub type UniqueNetIdToStringFn =
    unsafe extern "C" fn(unique_id: *const c_void, out_str: *mut FString) -> *mut FString;

/// Calling convention for `FReferenceControllerBase::DestroyObject(this)`.
pub type RefCtrlDestroyObjFn = unsafe extern "C" fn(controller: *mut c_void);

/// Calling convention for `FReferenceControllerBase::~FReferenceControllerBase(this, uint32 flags)`.
pub type RefCtrlDtorFn = unsafe extern "C" fn(controller: *mut c_void, flags: u32);

/// Calling convention for `IVoiceChatUser::SetAudioInputVolume(this, float Volume)`.
pub type SetAudioInputVolumeFn = unsafe extern "C" fn(voice_chat_user: *mut c_void, volume: f32);

/// Calling convention for `IVoiceChatUser::SetAudio{Input,Output}DeviceMuted(this, bool bMuted)`.
pub type SetAudioMutedFn = unsafe extern "C" fn(voice_chat_user: *mut c_void, muted: u8);

/// Calling convention for `IVoiceChatUser::GetAudio{Input,Output}DeviceMuted(this) -> bool`.
pub type GetAudioMutedFn = unsafe extern "C" fn(voice_chat_user: *mut c_void) -> u8;
