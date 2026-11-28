/// Client RVA of global `UEngine* GEngine`.
pub const RVA_GENGINE: usize = 0x04C7_A210;
/// Client RVA of `UGameEngine::GetGameWorld(UGameEngine* this) -> UWorld*`.
pub const RVA_UGAMEENGINE_GET_GAME_WORLD: usize = 0x028D_DE10;
/// Client RVA of `UGameInstance::GetFirstLocalPlayerController(UGameInstance*, UWorld*)`.
pub const RVA_UGAMEINSTANCE_GET_FIRST_LOCAL_PC: usize = 0x0290_54E0;

/// Client RVA of `UIpNetDriver::GetSocket(UIpNetDriver* this) -> FSocket*`.
pub const RVA_UIPNETDRIVER_GET_SOCKET: usize = 0x0090_9B80;
/// Client RVA of `UIpNetDriver::GetPrivateStaticClass() -> UClass*`.
pub const RVA_UIPNETDRIVER_STATIC_CLASS: usize = 0x0093_7370;

/// Client RVA of `ADH_GameState::GetPrivateStaticClass() -> UClass*`.
pub const RVA_ADH_GAMESTATE_STATIC_CLASS: usize = 0x00F3_5D80;
/// Client RVA of `UDH_GameInstance::GetPrivateStaticClass() -> UClass*`.
pub const RVA_UDH_GAMEINSTANCE_STATIC_CLASS: usize = 0x00F2_BA10;
/// Client RVA of `ADH_PlayerState::GetPrivateStaticClass() -> UClass*`.
pub const RVA_ADH_PLAYERSTATE_STATIC_CLASS: usize = 0x00F6_51F0;

/// Client RVA of `FMemory::Free(void* original)`.
pub const RVA_FMEMORY_FREE: usize = 0x010D_50F0;

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

/// `UObjectBase::ClassPrivate` offset (`UClass*`).
pub const OFFSET_UOBJECT_CLASS: usize = 0x10;

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

/// `FSocketBSD::SocketType` offset (`u32`: `0 = Unknown`, `1 = Datagram/UDP`, `2 = Streaming/TCP`).
pub const OFFSET_FSOCKET_BSD_SOCKET_TYPE: usize = 0x08;
/// `FSocketBSD::Socket` offset (Win32 `SOCKET` handle as `usize`).
pub const OFFSET_FSOCKET_BSD_RAW_SOCKET: usize = 0x28;
/// `FInternetAddrBSD::Addr` offset (`SOCKADDR_STORAGE`, 128 bytes).
pub const OFFSET_FINTERNET_ADDR_BSD_STORAGE: usize = 0x08;

/// `UDH_GameInstance::OnlineVoice` offset (`UDH_OnlineVoice*`, verified at `0xE0B620`).
pub const OFFSET_DH_GAME_INSTANCE_ONLINE_VOICE: usize = 0x520;
/// `UDH_OnlineVoice::bWantsToTalk` offset (`bool`, verified at `0xEB1F95`).
pub const OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK: usize = 0x80;
/// `UDH_OnlineVoice::CachedVoipMaxRange` offset (`float`, verified at `0xEB50FC`).
pub const OFFSET_DH_ONLINE_VOICE_MAX_RANGE: usize = 0x9C;

/// `AController::PlayerState` offset (`APlayerState*`, verified at `0xF07DEB`).
pub const OFFSET_CONTROLLER_PLAYER_STATE: usize = 0x228;

/// `APlayerState::UniqueId.UniqueNetId.Object` offset (`const FUniqueNetId*`, `0x250 + 0x08`).
pub const OFFSET_PS_UNIQUE_NET_ID_PTR: usize = 0x258;
/// Client `ADH_PlayerState::bIsMuted` offset (`bool`, includes manual + global mute, `0x1021F54`).
pub const OFFSET_PS_IS_MUTED: usize = 0x642;
/// Client `ADH_PlayerState::bIsDeadGhost` offset (`bool`, verified in `UpdateMuteStatus`).
pub const OFFSET_PS_IS_GHOST: usize = 0x646;
/// Client `ADH_PlayerState::bCanHearGhosts` offset (`bool`, verified in `UpdateMuteStatus`).
pub const OFFSET_PS_CAN_HEAR_GHOSTS: usize = 0x647;

/// `FUniqueNetId::IsValid` vtable byte offset (`0x28`, verified at `0x927B19`).
pub const VTABLE_OFF_UNIQUE_NET_ID_IS_VALID: usize = 0x28;
/// `FUniqueNetId::ToString` vtable byte offset (`0x30`, verified at `0x1021F1F`).
pub const VTABLE_OFF_UNIQUE_NET_ID_TO_STRING: usize = 0x30;

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

/// Active speaker indicator hold window (`350 ms`) after receiving non-silent decoded PCM.
pub const SPEAKER_ACTIVE_WINDOW_MS: u64 = 350;
/// Expiry window (`1500 ms`) for pruning stale speaker entries during periodic ticks.
pub const SPEAKER_PRUNE_WINDOW_MS: u64 = 1500;
/// Minimum peak `i16` sample amplitude treated as audible speech for HUD speaking indicators.
pub const SPEECH_PEAK_THRESHOLD: i16 = 32;
/// Maximum tracked remote speakers in the in-memory speaking map (hard cap).
pub const MAX_TRACKED_SPEAKERS: usize = 64;
/// Tick interval between periodic EOS mute sanity re-checks during a match.
pub const MATCH_REASSERT_TICK_INTERVAL: u32 = 64;
/// Default VOIP range fallback (UE units) if `UDH_OnlineVoice::CachedVoipMaxRange` is unset.
pub const FALLBACK_VOIP_MAX_RANGE: f32 = 3500.0;
