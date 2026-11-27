use std::ffi::CStr;

/// Client RVA of global `UEngine* GEngine`.
pub const RVA_GENGINE: usize = 0x04C7_A210;
/// Client RVA of `UGameEngine::GetGameWorld(UGameEngine* this) -> UWorld*`.
pub const RVA_UGAMEENGINE_GET_GAME_WORLD: usize = 0x028D_DE10;
/// Client RVA of `UGameInstance::GetFirstLocalPlayerController(UGameInstance*, UWorld*)`.
pub const RVA_UGAMEINSTANCE_GET_FIRST_LOCAL_PC: usize = 0x0290_54E0;

/// Client RVA of `ADH_GameState::GetPrivateStaticClass() -> UClass*`.
pub const RVA_ADH_GAMESTATE_STATIC_CLASS: usize = 0x00F3_5D80;
/// Client RVA of `UDH_GameInstance::GetPrivateStaticClass() -> UClass*`.
pub const RVA_UDH_GAMEINSTANCE_STATIC_CLASS: usize = 0x00F2_BA10;
/// Client RVA of `ADH_PlayerState::GetPrivateStaticClass() -> UClass*`.
pub const RVA_ADH_PLAYERSTATE_STATIC_CLASS: usize = 0x00F6_51F0;

/// Client RVA of `FNameEntryId::FromValidEName(FNameEntryId* out, EName ename)`.
pub const RVA_FNAME_ENTRY_ID_FROM_VALID_ENAME: usize = 0x011D_7350;
/// Client RVA of `FName::FName(FName* this, const ANSICHAR* name, EFindName find_type)`.
pub const RVA_FNAME_CTOR_ANSI: usize = 0x011D_0730;
/// Client RVA of `FMemory::Realloc(void* original, SIZE_T count, uint32 alignment)`.
pub const RVA_FMEMORY_REALLOC: usize = 0x010E_3990;
/// Client RVA of `FMemory::Free(void* original)`.
pub const RVA_FMEMORY_FREE: usize = 0x010D_50F0;

/// Client RVA of `FOnlineVoiceImpl::RegisterLocalTalker(this, uint32 LocalUserNum) -> bool`.
pub const RVA_ONLINE_VOICE_REGISTER_LOCAL_TALKER: usize = 0x0091_EE90;
/// Client RVA of `FOnlineVoiceImpl::StartNetworkedVoice(this, uint8 LocalUserNum)`.
pub const RVA_ONLINE_VOICE_START_NETWORKED_VOICE: usize = 0x0092_7010;
/// Client RVA of `FOnlineVoiceImpl::StopNetworkedVoice(this, uint8 LocalUserNum)`.
pub const RVA_ONLINE_VOICE_STOP_NETWORKED_VOICE: usize = 0x0092_7720;

/// Client RVA of `UDH_OnlineVoice::BeforeRecvAudioRendered(UDH_OnlineVoice**, FString*, TArrayView<i16>*)`.
pub const RVA_DH_ONLINE_VOICE_BEFORE_RECV_AUDIO: usize = 0x00E9_F9C0;
/// Client RVA of `UDH_OnlineVoice::GetVoiceChatUser(UDH_OnlineVoice* this) -> IVoiceChatUser*`.
pub const RVA_DH_ONLINE_VOICE_GET_VOICE_CHAT_USER: usize = 0x00EB_1840;
/// Client RVA of `UDH_OnlineVoice::IsInVoipRange(this, ADH_PlayerState*, float MaxRange) -> bool`.
pub const RVA_DH_ONLINE_VOICE_IS_IN_VOIP_RANGE: usize = 0x00EB_4DB0;
/// Client RVA of `UDH_GameplayStatics::GetDHPlayerStateFromStringUniqueId(FString*, UObject*)`.
pub const RVA_DH_GET_PS_FROM_UNIQUE_ID_STR: usize = 0x00F0_6B80;
/// Client RVA of `UDH_GameplayStatics::GetLocalDHPlayerState(UObject*)`.
pub const RVA_DH_GET_LOCAL_PLAYER_STATE: usize = 0x00F0_7D90;

/// `UObjectBase::ObjectFlags` offset (`uint32`).
pub const OFFSET_UOBJECT_FLAGS: usize = 0x08;
/// `UObjectBase::ClassPrivate` offset (`UClass*`).
pub const OFFSET_UOBJECT_CLASS: usize = 0x10;
/// `RF_ClassDefaultObject` bitmask (`1 << 4`).
pub const UOBJECT_FLAG_CDO: u32 = 0x0000_0010;

/// `FStructBaseChain::StructBaseChainArray` offset inside `UStruct`/`UClass`.
pub const OFFSET_UCLASS_BASE_CHAIN_ARRAY: usize = 0x30;
/// `FStructBaseChain::NumStructBasesInChainMinusOne` offset inside `UStruct`/`UClass`.
pub const OFFSET_UCLASS_BASE_CHAIN_DEPTH: usize = 0x38;

/// `UWorld::NetDriver` offset (`UNetDriver*`).
pub const OFFSET_UWORLD_NET_DRIVER: usize = 0x38;
/// `UWorld::GameState` offset (`AGameStateBase*`).
pub const OFFSET_UWORLD_GAME_STATE: usize = 0x120;
/// `UWorld::OwningGameInstance` offset (`UGameInstance*`).
pub const OFFSET_UWORLD_OWNING_GAME_INSTANCE: usize = 0x180;

/// `UNetDriver::ServerConnection` offset (`UNetConnection*`).
pub const OFFSET_NET_DRIVER_SERVER_CONNECTION: usize = 0x88;
/// `UNetDriver::ChannelDefinitions` offset (`TArray<FChannelDefinition>`).
pub const OFFSET_NET_DRIVER_CHANNEL_DEFINITIONS: usize = 0x198;

/// `UDH_GameInstance::OnlineVoice` offset (`UDH_OnlineVoice*`, verified at `0xE0B620`).
pub const OFFSET_DH_GAME_INSTANCE_ONLINE_VOICE: usize = 0x520;
/// `UDH_OnlineVoice::bWantsToTalk` offset (`bool`, verified at `0xEB1F95`).
pub const OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK: usize = 0x80;
/// `UDH_OnlineVoice::CachedVoipMaxRange` offset (`float`, verified at `0xEB50FC`).
pub const OFFSET_DH_ONLINE_VOICE_MAX_RANGE: usize = 0x9C;

/// `AController::PlayerState` offset (`APlayerState*`, verified at `0xF07DEB`).
pub const OFFSET_CONTROLLER_PLAYER_STATE: usize = 0x228;
/// `APlayerController::bVoiceHandshakeComplete` offset (`bool`, verified at `0x2B61F60`).
pub const OFFSET_PC_VOICE_HANDSHAKE_COMPLETE: usize = 0x408;

/// `APlayerState::UniqueId.UniqueNetId.Object` offset (`const FUniqueNetId*`, `0x250 + 0x08`).
pub const OFFSET_PS_UNIQUE_NET_ID_PTR: usize = 0x258;
/// Client `ADH_PlayerState::bIsMuted` offset (`bool`, includes manual + global mute, `0x1021F54`).
pub const OFFSET_PS_IS_MUTED: usize = 0x642;
/// Client `ADH_PlayerState::bIsDeadGhost` offset (`bool`, verified in `UpdateMuteStatus`).
pub const OFFSET_PS_IS_GHOST: usize = 0x646;
/// Client `ADH_PlayerState::bCanHearGhosts` offset (`bool`, verified in `UpdateMuteStatus`).
pub const OFFSET_PS_CAN_HEAR_GHOSTS: usize = 0x647;

/// `FOnlineSubsystemEOSPlus::BasePlatformInterface` (`FOnlineSubsystemSteam*`, `0xA284A0`).
pub const OFFSET_EOS_PLUS_BASE_PLATFORM: usize = 0xB8;
/// `FOnlineSubsystemEOSPlus::UserManager` (`FOnlineUserEOSPlus*`, `0xA24450`).
pub const OFFSET_EOS_PLUS_USER_MANAGER: usize = 0xE8;
/// Offset from `FOnlineUserEOSPlus*` to its `IOnlineIdentity` interface subobject (`0xA2445D`).
pub const OFFSET_EOS_PLUS_USER_MANAGER_IDENTITY: usize = 0x68;
/// `IOnlineSubsystem::GetVoiceInterface` vtable byte offset (`slot 13 = 0x68`).
pub const VTABLE_OFF_OSS_GET_VOICE_INTERFACE: usize = 0x68;

/// `FOnlineVoiceImpl::IdentityInt` offset (`IOnlineIdentity*`, verified at `0x91B87D`).
pub const OFFSET_ONLINE_VOICE_IMPL_IDENTITY_INT: usize = 0x30;
/// `IOnlineIdentity::GetUniquePlayerId` vtable byte offset (`0xF0`, verified at `0x91B89B`).
pub const VTABLE_OFF_IDENTITY_GET_UNIQUE_PLAYER_ID: usize = 0xF0;

/// `FUniqueNetIdWrapper::UniqueNetId.Object` offset (`const FUniqueNetId*`, verified at `0x927B0D`).
pub const OFFSET_UNIQUE_NET_ID_WRAPPER_PTR: usize = 0x08;
/// `FUniqueNetId::IsValid` vtable byte offset (`0x28`, verified at `0x927B19`).
pub const VTABLE_OFF_UNIQUE_NET_ID_IS_VALID: usize = 0x28;
/// `FUniqueNetId::ToString` vtable byte offset (`0x30`, verified at `0x1021F1F`).
pub const VTABLE_OFF_UNIQUE_NET_ID_TO_STRING: usize = 0x30;

/// `FReferenceControllerBase::SharedReferenceCount` offset (`i32`).
pub const OFFSET_REF_CTRL_SHARED_COUNT: usize = 0x08;
/// `FReferenceControllerBase::WeakReferenceCount` offset (`i32`).
pub const OFFSET_REF_CTRL_WEAK_COUNT: usize = 0x0C;
/// `FReferenceControllerBase::DestroyObject` vtable byte offset (`0x00`).
pub const VTABLE_OFF_REF_CTRL_DESTROY_OBJECT: usize = 0x00;
/// `FReferenceControllerBase::~FReferenceControllerBase` deleting dtor vtable byte offset (`0x08`).
pub const VTABLE_OFF_REF_CTRL_DTOR: usize = 0x08;

/// `IVoiceChatUser::SetAudioInputVolume` vtable byte offset (`0x18`).
pub const VTABLE_OFF_VOICE_CHAT_SET_INPUT_VOLUME: usize = 0x18;
/// `IVoiceChatUser::SetAudioInputDeviceMuted` vtable byte offset (`0x38`).
pub const VTABLE_OFF_VOICE_CHAT_SET_INPUT_MUTED: usize = 0x38;
/// `IVoiceChatUser::SetAudioOutputDeviceMuted` vtable byte offset (`0x40`).
pub const VTABLE_OFF_VOICE_CHAT_SET_OUTPUT_MUTED: usize = 0x40;
/// `IVoiceChatUser::GetAudioInputDeviceMuted` vtable byte offset (`0x48`).
pub const VTABLE_OFF_VOICE_CHAT_GET_INPUT_MUTED: usize = 0x48;
/// `IVoiceChatUser::GetAudioOutputDeviceMuted` vtable byte offset (`0x50`).
pub const VTABLE_OFF_VOICE_CHAT_GET_OUTPUT_MUTED: usize = 0x50;

/// Built-in UE4 `EName::NAME_Voice` enumerator value (`256`).
pub const ENAME_VOICE: u32 = 0x0100;
/// `EFindName::FNAME_Add` (`1`).
pub const FNAME_ADD: u32 = 1;
/// UE4 default static channel index for `Voice` (`Control = 0`, `Voice = 1`, `Actor = -1`).
pub const VOICE_DEFAULT_STATIC_CHANNEL_INDEX: i32 = 1;
/// Fallback dynamic channel index when slot `1` is already occupied.
pub const DYNAMIC_STATIC_CHANNEL_INDEX: i32 = -1;
/// Upper sanity bound on `UNetDriver::ChannelDefinitions` element count.
pub const MAX_SANE_CHANNEL_DEFINITIONS: i32 = 256;
/// Minimum capacity allocated when growing `UNetDriver::ChannelDefinitions`.
pub const MIN_CHANNEL_DEFINITIONS_CAPACITY: i32 = 4;

/// Class path for UE4's built-in `UVoiceChannel` used in `FChannelDefinition::ClassName`.
pub const VOICE_CHANNEL_CLASS_PATH: &CStr = c"/Script/Engine.VoiceChannel";

/// Active speaker indicator hold window (`350 ms`) after receiving non-silent decoded PCM.
pub const SPEAKER_ACTIVE_WINDOW_MS: u64 = 350;
/// Expiry window (`1500 ms`) for pruning stale speaker entries during periodic ticks.
pub const SPEAKER_PRUNE_WINDOW_MS: u64 = 1500;
/// Minimum peak `i16` sample amplitude treated as audible speech for HUD speaking indicators.
pub const SPEECH_PEAK_THRESHOLD: i16 = 32;
/// Maximum tracked remote speakers in the in-memory speaking map (hard cap).
pub const MAX_TRACKED_SPEAKERS: usize = 64;
/// Tick interval between periodic EOS mute / identity sanity re-checks during a match.
pub const MATCH_REASSERT_TICK_INTERVAL: u32 = 64;
/// Default VOIP range fallback (UE units) if `UDH_OnlineVoice::CachedVoipMaxRange` is unset.
pub const FALLBACK_VOIP_MAX_RANGE: f32 = 3500.0;
