use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::consts::ue4::{
    DYNAMIC_STATIC_CHANNEL_INDEX, ENAME_VOICE, FALLBACK_VOIP_MAX_RANGE, FNAME_ADD,
    MATCH_REASSERT_TICK_INTERVAL, MAX_SANE_CHANNEL_DEFINITIONS, MAX_TRACKED_SPEAKERS,
    MIN_CHANNEL_DEFINITIONS_CAPACITY, OFFSET_CONTROLLER_PLAYER_STATE,
    OFFSET_DH_GAME_INSTANCE_ONLINE_VOICE, OFFSET_DH_ONLINE_VOICE_MAX_RANGE,
    OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK, OFFSET_EOS_PLUS_BASE_PLATFORM,
    OFFSET_EOS_PLUS_USER_MANAGER, OFFSET_EOS_PLUS_USER_MANAGER_IDENTITY,
    OFFSET_NET_DRIVER_CHANNEL_DEFINITIONS, OFFSET_NET_DRIVER_SERVER_CONNECTION,
    OFFSET_ONLINE_VOICE_IMPL_IDENTITY_INT, OFFSET_PC_VOICE_HANDSHAKE_COMPLETE,
    OFFSET_PS_CAN_HEAR_GHOSTS, OFFSET_PS_IS_GHOST, OFFSET_PS_IS_MUTED, OFFSET_PS_UNIQUE_NET_ID_PTR,
    OFFSET_REF_CTRL_SHARED_COUNT, OFFSET_REF_CTRL_WEAK_COUNT, OFFSET_UCLASS_BASE_CHAIN_ARRAY,
    OFFSET_UCLASS_BASE_CHAIN_DEPTH, OFFSET_UNIQUE_NET_ID_WRAPPER_PTR, OFFSET_UOBJECT_CLASS,
    OFFSET_UOBJECT_FLAGS, OFFSET_UWORLD_GAME_STATE, OFFSET_UWORLD_NET_DRIVER,
    OFFSET_UWORLD_OWNING_GAME_INSTANCE, RVA_ADH_GAMESTATE_STATIC_CLASS,
    RVA_ADH_PLAYERSTATE_STATIC_CLASS, RVA_DH_GET_LOCAL_PLAYER_STATE,
    RVA_DH_GET_PS_FROM_UNIQUE_ID_STR, RVA_DH_ONLINE_VOICE_BEFORE_RECV_AUDIO,
    RVA_DH_ONLINE_VOICE_GET_VOICE_CHAT_USER, RVA_DH_ONLINE_VOICE_IS_IN_VOIP_RANGE,
    RVA_FMEMORY_FREE, RVA_FMEMORY_REALLOC, RVA_FNAME_CTOR_ANSI,
    RVA_FNAME_ENTRY_ID_FROM_VALID_ENAME, RVA_GENGINE, RVA_ONLINE_VOICE_REGISTER_LOCAL_TALKER,
    RVA_ONLINE_VOICE_START_NETWORKED_VOICE, RVA_ONLINE_VOICE_STOP_NETWORKED_VOICE,
    RVA_UDH_GAMEINSTANCE_STATIC_CLASS, RVA_UGAMEENGINE_GET_GAME_WORLD,
    RVA_UGAMEINSTANCE_GET_FIRST_LOCAL_PC, SPEAKER_ACTIVE_WINDOW_MS, SPEAKER_PRUNE_WINDOW_MS,
    SPEECH_PEAK_THRESHOLD, UOBJECT_FLAG_CDO, VOICE_CHANNEL_CLASS_PATH,
    VOICE_DEFAULT_STATIC_CHANNEL_INDEX, VTABLE_OFF_IDENTITY_GET_UNIQUE_PLAYER_ID,
    VTABLE_OFF_OSS_GET_VOICE_INTERFACE, VTABLE_OFF_REF_CTRL_DESTROY_OBJECT,
    VTABLE_OFF_REF_CTRL_DTOR, VTABLE_OFF_UNIQUE_NET_ID_IS_VALID,
    VTABLE_OFF_UNIQUE_NET_ID_TO_STRING, VTABLE_OFF_VOICE_CHAT_GET_INPUT_MUTED,
    VTABLE_OFF_VOICE_CHAT_GET_OUTPUT_MUTED, VTABLE_OFF_VOICE_CHAT_SET_INPUT_MUTED,
    VTABLE_OFF_VOICE_CHAT_SET_INPUT_VOLUME, VTABLE_OFF_VOICE_CHAT_SET_OUTPUT_MUTED,
};
use crate::logger::log_line;
use crate::structs::state::{MatchContext, SpeakerEntry};
use crate::structs::ue4::{
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

static MODULE_BASE: AtomicUsize = AtomicUsize::new(0);

/// Single-byte flag read directly by the naked `FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered`
/// guard stub (`1` while connected to a Dedicated Server match, `0` in Main Menu or Lobby).
pub static IN_DEDICATED_MATCH: AtomicBool = AtomicBool::new(false);

static EOS_PLUS_PTR: AtomicUsize = AtomicUsize::new(0);
static ONLINE_VOICE_IMPL_PTR: AtomicUsize = AtomicUsize::new(0);
static ORIG_STEAM_IDENTITY_INT: AtomicUsize = AtomicUsize::new(0);
static MATCH_TICK_COUNTER: AtomicU32 = AtomicU32::new(0);

static ORIG_LOAD_CHANNEL_DEFINITIONS: AtomicUsize = AtomicUsize::new(0);
static ORIG_ONLINE_VOICE_TICK: AtomicUsize = AtomicUsize::new(0);
static ORIG_SET_WANTS_TO_TALK: AtomicUsize = AtomicUsize::new(0);
static ORIG_GET_WANTS_TO_TALK: AtomicUsize = AtomicUsize::new(0);
static ORIG_IS_PARTICIPANT_SPEAKING: AtomicUsize = AtomicUsize::new(0);
static ORIG_IS_PLAYER_IN_VOICE_CHAT: AtomicUsize = AtomicUsize::new(0);

static ACTIVE_SPEAKERS: Mutex<Vec<SpeakerEntry>> = Mutex::new(Vec::new());

/// Stores the verified `DreadHunger-Win64-Shipping.exe` base address before installing hooks.
pub fn set_module_base(module_base: usize) {
    MODULE_BASE.store(module_base, Ordering::Release);
}

/// Registers the original-call trampoline address for `UNetDriver::LoadChannelDefinitions`.
pub fn set_orig_load_channel_definitions(addr: usize) {
    ORIG_LOAD_CHANNEL_DEFINITIONS.store(addr, Ordering::Release);
}

/// Registers the original-call trampoline address for `FOnlineVoiceImpl::Tick`.
pub fn set_orig_online_voice_tick(addr: usize) {
    ORIG_ONLINE_VOICE_TICK.store(addr, Ordering::Release);
}

/// Registers the original-call trampoline address for `UDH_OnlineVoice::SetWantsToTalk`.
pub fn set_orig_set_wants_to_talk(addr: usize) {
    ORIG_SET_WANTS_TO_TALK.store(addr, Ordering::Release);
}

/// Registers the original-call trampoline address for `UDH_OnlineVoice::GetWantsToTalk`.
pub fn set_orig_get_wants_to_talk(addr: usize) {
    ORIG_GET_WANTS_TO_TALK.store(addr, Ordering::Release);
}

/// Registers the original-call trampoline address for `UDH_OnlineVoice::IsParticipantSpeaking`.
pub fn set_orig_is_participant_speaking(addr: usize) {
    ORIG_IS_PARTICIPANT_SPEAKING.store(addr, Ordering::Release);
}

/// Registers the original-call trampoline address for `UDH_OnlineVoice::IsPlayerInVoiceChat`.
pub fn set_orig_is_player_in_voice_chat(addr: usize) {
    ORIG_IS_PLAYER_IN_VOICE_CHAT.store(addr, Ordering::Release);
}

/// Vtable replacement for `FOnlineSubsystemEOSPlus::GetVoiceInterface` (slot 13, `+0x68`).
///
/// Caches `this_eos_plus` for later `EOSPlus->GetIdentityInterface()` lookup and delegates
/// voice interface creation/retrieval to `BasePlatformInterface` (`FOnlineSubsystemSteam`).
///
/// # Safety
/// Must only be called through the patched `FOnlineSubsystemEOSPlus` vtable slot with a valid
/// `FOnlineSubsystemEOSPlus*` (or null) and a writable caller-owned `out_shared_ptr`.
pub unsafe extern "C" fn detour_eos_plus_get_voice_interface(
    this_eos_plus: *mut c_void,
    out_shared_ptr: *mut SharedPtr,
) -> *mut SharedPtr {
    if out_shared_ptr.is_null() {
        return out_shared_ptr;
    }

    if this_eos_plus.is_null() {
        // SAFETY: `out_shared_ptr` is non-null caller-supplied return buffer.
        unsafe {
            (*out_shared_ptr).object = std::ptr::null_mut();
            (*out_shared_ptr).controller = std::ptr::null_mut();
        }
        return out_shared_ptr;
    }

    EOS_PLUS_PTR.store(this_eos_plus as usize, Ordering::Release);

    // SAFETY: `FOnlineSubsystemEOSPlus::BasePlatformInterface` (`FOnlineSubsystemSteam*`) is at `+0xB8`.
    let base_platform: *mut c_void = unsafe {
        *(this_eos_plus
            .byte_add(OFFSET_EOS_PLUS_BASE_PLATFORM)
            .cast::<*mut c_void>())
    };
    if base_platform.is_null() {
        // SAFETY: Zeroing caller-supplied `TSharedPtr` when base platform subsystem is unavailable.
        unsafe {
            (*out_shared_ptr).object = std::ptr::null_mut();
            (*out_shared_ptr).controller = std::ptr::null_mut();
        }
        return out_shared_ptr;
    }

    // SAFETY: Reading vtable slot `+0x68` (`IOnlineSubsystem::GetVoiceInterface`) of `FOnlineSubsystemSteam`.
    let vtable: *const usize = unsafe { *(base_platform.cast::<*const usize>()) };
    if vtable.is_null() {
        // SAFETY: Zeroing caller-supplied `TSharedPtr`.
        unsafe {
            (*out_shared_ptr).object = std::ptr::null_mut();
            (*out_shared_ptr).controller = std::ptr::null_mut();
        }
        return out_shared_ptr;
    }
    // SAFETY: Slot index `0x68 / 8 = 13` is `IOnlineSubsystem::GetVoiceInterface`.
    let get_voice_addr: usize = unsafe { *vtable.add(VTABLE_OFF_OSS_GET_VOICE_INTERFACE / 8) };
    // SAFETY: Verified virtual method signature matching `IOnlineSubsystem::GetVoiceInterface`.
    let get_voice_fn: GetVoiceInterfaceFn =
        unsafe { std::mem::transmute::<usize, GetVoiceInterfaceFn>(get_voice_addr) };
    // SAFETY: Delegating directly to `FOnlineSubsystemSteam::GetVoiceInterface`.
    let ret: *mut SharedPtr = unsafe { get_voice_fn(base_platform, out_shared_ptr) };

    // SAFETY: `ret` points to the populated `SharedPtr`.
    let voice_obj: *mut c_void = unsafe { (*ret).object };
    if !voice_obj.is_null() {
        ONLINE_VOICE_IMPL_PTR.store(voice_obj as usize, Ordering::Release);
    }

    ret
}

/// Detour entry point for `UNetDriver::LoadChannelDefinitions(UNetDriver* this)`.
///
/// # Safety
/// Must only be invoked by UE4 through the verified `UNetDriver::LoadChannelDefinitions` detour
/// on the game thread with a live `UNetDriver*` (or null).
pub unsafe extern "C" fn detour_load_channel_definitions(net_driver: *mut c_void) {
    let orig_addr: usize = ORIG_LOAD_CHANNEL_DEFINITIONS.load(Ordering::Acquire);
    if orig_addr == 0 {
        return;
    }

    if !net_driver.is_null() {
        // SAFETY: `net_driver` is a live `UNetDriver*` on the game thread.
        unsafe {
            ensure_voice_channel_definition(net_driver);
        }
    }

    // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
    let orig_fn: LoadChannelDefinitionsFn =
        unsafe { std::mem::transmute::<usize, LoadChannelDefinitionsFn>(orig_addr) };
    // SAFETY: Forwarding to original `UNetDriver::LoadChannelDefinitions`.
    unsafe {
        orig_fn(net_driver);
    }
}

/// Detour entry point for `FOnlineVoiceImpl::Tick(FOnlineVoiceImpl* this, float DeltaTime)`.
///
/// Implements the Plan A dual-mode state machine on the UE4 Game Thread:
/// - Main Menu / Lobby (`ADH_LobbyGameState` or no `ServerConnection`): keeps vanilla EOS Voice
///   fully active and does not tick `FOnlineVoiceImpl` (zero mic contention or CPU cost).
/// - Dedicated Server (`ADH_GameState` + `NetDriver->ServerConnection != null`): silences EOS RTC
///   input/output, routes wire IDs via `EOSPlus` (`"SteamID|EOS_PUID"`), ensures local talker
///   registration & voice handshake flag, and ticks native UE4 48 kHz Opus voice.
///
/// # Safety
/// Must only be invoked by UE4 through the verified `FOnlineVoiceImpl::Tick` detour on the game
/// thread with a valid `FOnlineVoiceImpl*`.
pub unsafe extern "C" fn detour_online_voice_tick(voice_impl: *mut c_void, delta_time: f32) {
    let orig_addr: usize = ORIG_ONLINE_VOICE_TICK.load(Ordering::Acquire);
    if orig_addr == 0 || voice_impl.is_null() {
        return;
    }

    ONLINE_VOICE_IMPL_PTR.store(voice_impl as usize, Ordering::Release);

    let base: usize = MODULE_BASE.load(Ordering::Acquire);
    if base == 0 {
        return;
    }

    // SAFETY: Game-thread inspection of live `GEngine`/`UWorld`/`UNetDriver`/`ADH_GameState`.
    let match_ctx_opt: Option<MatchContext> = unsafe { query_match_context(base) };
    let was_in_match: bool = IN_DEDICATED_MATCH.load(Ordering::Acquire);

    if let Some(ctx) = match_ctx_opt {
        if !ctx.local_pc.is_null() {
            // SAFETY: `APlayerController::bVoiceHandshakeComplete` is verified at `+0x408`.
            unsafe {
                *ctx.local_pc
                    .byte_add(OFFSET_PC_VOICE_HANDSHAKE_COMPLETE)
                    .cast::<u8>() = 1;
            }
        }

        if was_in_match {
            let tick_num: u32 = MATCH_TICK_COUNTER
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_add(1);
            if tick_num.is_multiple_of(MATCH_REASSERT_TICK_INTERVAL) {
                // SAFETY: Periodic game-thread maintenance (re-assert EOS silence & prune speakers).
                unsafe {
                    maintain_dedicated_match_mode(base, voice_impl, &ctx);
                }
            }
        } else {
            IN_DEDICATED_MATCH.store(true, Ordering::Release);
            MATCH_TICK_COUNTER.store(0, Ordering::Release);
            // SAFETY: Entering Dedicated Server match mode on the game thread.
            unsafe {
                enter_dedicated_match_mode(base, voice_impl, &ctx);
            }
        }

        // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
        let orig_fn: OnlineVoiceTickFn =
            unsafe { std::mem::transmute::<usize, OnlineVoiceTickFn>(orig_addr) };
        // SAFETY: Ticking native UE4 `FOnlineVoiceImpl` while on a Dedicated Server.
        unsafe {
            orig_fn(voice_impl, delta_time);
        }
    } else if was_in_match {
        IN_DEDICATED_MATCH.store(false, Ordering::Release);
        // SAFETY: Exiting Dedicated Server back to Lobby/Main Menu on the game thread.
        unsafe {
            exit_dedicated_match_mode(base, voice_impl);
        }
    }
}

/// Callback invoked from our `54`-byte naked stub inside `FVoiceEngineImpl::SubmitRemoteVoiceData`
/// (`0x927C0C`) immediately after `FVoiceDecoderOpus::Decode` produces 48 kHz mono `int16` PCM.
///
/// Enforces manual/global mute and ghost/living voice isolation, updates the speaking indicator
/// tracker, and injects PCM directly into `UDH_OnlineVoice::BeforeRecvAudioRendered` (`0xE9F9C0`).
///
/// # Safety
/// Must only be invoked from our `SubmitRemoteVoiceData` trampoline stub with a valid
/// `FUniqueNetIdWrapper*` and a readable 16-bit PCM buffer of at least `pcm_byte_len` bytes.
pub unsafe extern "C" fn detour_on_decoded_remote_pcm(
    remote_talker_wrapper: *const c_void,
    pcm_data_ptr: *const c_void,
    pcm_byte_len: u32,
    _voice_engine: *mut c_void,
) {
    if !IN_DEDICATED_MATCH.load(Ordering::Acquire)
        || remote_talker_wrapper.is_null()
        || pcm_data_ptr.is_null()
        || pcm_byte_len < 2
    {
        return;
    }

    let base: usize = MODULE_BASE.load(Ordering::Acquire);
    if base == 0 {
        return;
    }

    // SAFETY: Game-thread query for active Dedicated Server world and `UDH_OnlineVoice*`.
    let Some(ctx): Option<MatchContext> = (unsafe { query_match_context(base) }) else {
        return;
    };
    if ctx.online_voice.is_null() {
        return;
    }

    // SAFETY: `FUniqueNetIdWrapper::UniqueNetId.Object` (`const FUniqueNetId*`) is at `+0x08`.
    let unique_net_id: *const c_void = unsafe {
        *(remote_talker_wrapper
            .byte_add(OFFSET_UNIQUE_NET_ID_WRAPPER_PTR)
            .cast::<*const c_void>())
    };
    if unique_net_id.is_null() {
        return;
    }

    let total_samples: u32 = pcm_byte_len / 2;
    let Ok(sample_len_i32): Result<i32, std::num::TryFromIntError> = i32::try_from(total_samples)
    else {
        return;
    };
    let Ok(sample_len_usize): Result<usize, std::num::TryFromIntError> =
        usize::try_from(total_samples)
    else {
        return;
    };
    if sample_len_i32 <= 0 {
        return;
    }

    // SAFETY: `unique_net_id` is a valid `const FUniqueNetId*` for the duration of this frame.
    let _handled: bool = unsafe {
        with_unique_net_id_fstring(base, unique_net_id, |sender_fstr: &FString| {
            let target_ps: *mut c_void =
                call_get_dh_ps_from_unique_id(base, sender_fstr, ctx.online_voice);
            if !target_ps.is_null() {
                if target_ps == ctx.local_ps {
                    return;
                }
                let is_muted: u8 = *target_ps.byte_add(OFFSET_PS_IS_MUTED).cast::<u8>();
                if is_muted != 0 {
                    return;
                }
                if !ctx.local_ps.is_null() {
                    let local_can_hear_ghosts: u8 = *ctx
                        .local_ps
                        .byte_add(OFFSET_PS_CAN_HEAR_GHOSTS)
                        .cast::<u8>();
                    let target_is_ghost: u8 = *target_ps.byte_add(OFFSET_PS_IS_GHOST).cast::<u8>();
                    if local_can_hear_ghosts == 0 && target_is_ghost != 0 {
                        return;
                    }
                }
            }

            let pcm_i16_ptr: *const i16 = pcm_data_ptr.cast::<i16>();
            let pcm_slice: &[i16] = std::slice::from_raw_parts(pcm_i16_ptr, sample_len_usize);
            if has_audible_speech(pcm_slice) {
                if let Some(id_utf16) = fstring_to_utf16_vec(sender_fstr) {
                    record_active_speaker(id_utf16);
                }
            }

            let online_voice_slot: *mut c_void = ctx.online_voice;
            let pcm_view: TArrayViewI16 = TArrayViewI16 {
                data: pcm_i16_ptr,
                num: sample_len_i32,
                pad: 0,
            };
            call_before_recv_audio_rendered(
                base,
                &raw const online_voice_slot,
                sender_fstr,
                &raw const pcm_view,
            );
        })
    };
}

/// Detour entry point for `UDH_OnlineVoice::SetWantsToTalk(UDH_OnlineVoice* this, bool bWants)`.
///
/// # Safety
/// Must only be invoked by UE4 through the verified `UDH_OnlineVoice::SetWantsToTalk` detour on
/// the game thread with a live `UDH_OnlineVoice*` (or null).
pub unsafe extern "C" fn detour_set_wants_to_talk(online_voice: *mut c_void, wants_to_talk: u8) {
    let base: usize = MODULE_BASE.load(Ordering::Acquire);
    let in_dedicated: bool = IN_DEDICATED_MATCH.load(Ordering::Acquire)
        || (base != 0
            // SAFETY: Fallback check if PTT fires on the first frame before `FOnlineVoiceImpl::Tick`.
            && unsafe { query_match_context(base).is_some() });

    if !in_dedicated {
        let orig_addr: usize = ORIG_SET_WANTS_TO_TALK.load(Ordering::Acquire);
        if orig_addr != 0 {
            // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
            let orig_fn: SetWantsToTalkFn =
                unsafe { std::mem::transmute::<usize, SetWantsToTalkFn>(orig_addr) };
            // SAFETY: Forwarding Lobby/Menu PTT state directly to vanilla EOS implementation.
            unsafe {
                orig_fn(online_voice, wants_to_talk);
            }
        }
        return;
    }

    if online_voice.is_null() || base == 0 {
        return;
    }

    let wants_flag: u8 = u8::from(wants_to_talk != 0);
    // SAFETY: `UDH_OnlineVoice::bWantsToTalk` is verified at `+0x80`.
    unsafe {
        *online_voice
            .byte_add(OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK)
            .cast::<u8>() = wants_flag;
    }

    let voice_impl_addr: usize = ONLINE_VOICE_IMPL_PTR.load(Ordering::Acquire);
    if voice_impl_addr != 0 {
        let voice_impl: *mut c_void = voice_impl_addr as *mut c_void;
        // SAFETY: Driving local microphone capture through UE4 native `FOnlineVoiceImpl`.
        unsafe {
            call_register_local_talker(base, voice_impl);
            if wants_flag != 0 {
                call_start_networked_voice(base, voice_impl);
            } else {
                call_stop_networked_voice(base, voice_impl);
            }
        }
    }

    // SAFETY: Keep EOS RTC input muted while on a Dedicated Server.
    unsafe {
        silence_eos_voice_chat_user(base, online_voice);
    }
}

/// Detour entry point for `UDH_OnlineVoice::GetWantsToTalk(UDH_OnlineVoice* this) -> bool`.
///
/// # Safety
/// Must only be invoked by UE4 through the verified `UDH_OnlineVoice::GetWantsToTalk` detour on
/// the game thread with a live `UDH_OnlineVoice*` (or null).
pub unsafe extern "C" fn detour_get_wants_to_talk(online_voice: *mut c_void) -> u8 {
    if !IN_DEDICATED_MATCH.load(Ordering::Acquire) {
        let orig_addr: usize = ORIG_GET_WANTS_TO_TALK.load(Ordering::Acquire);
        if orig_addr == 0 {
            return 0;
        }
        // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
        let orig_fn: GetWantsToTalkFn =
            unsafe { std::mem::transmute::<usize, GetWantsToTalkFn>(orig_addr) };
        // SAFETY: Forwarding Lobby/Menu query directly to vanilla EOS implementation.
        return unsafe { orig_fn(online_voice) };
    }

    if online_voice.is_null() {
        return 0;
    }
    // SAFETY: `UDH_OnlineVoice::bWantsToTalk` is verified at `+0x80`.
    let flag: u8 = unsafe {
        *online_voice
            .byte_add(OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK)
            .cast::<u8>()
    };
    u8::from(flag != 0)
}

/// Detour entry point for `UDH_OnlineVoice::IsParticipantSpeaking(this, ADH_PlayerState*, bool)`.
///
/// # Safety
/// Must only be invoked by UE4 through the verified `UDH_OnlineVoice::IsParticipantSpeaking`
/// detour on the game thread with live `UDH_OnlineVoice*` and `ADH_PlayerState*` pointers (or null).
pub unsafe extern "C" fn detour_is_participant_speaking(
    online_voice: *mut c_void,
    player_state: *mut c_void,
    check_range: u8,
) -> u8 {
    if !IN_DEDICATED_MATCH.load(Ordering::Acquire) {
        let orig_addr: usize = ORIG_IS_PARTICIPANT_SPEAKING.load(Ordering::Acquire);
        if orig_addr == 0 {
            return 0;
        }
        // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
        let orig_fn: IsParticipantSpeakingFn =
            unsafe { std::mem::transmute::<usize, IsParticipantSpeakingFn>(orig_addr) };
        // SAFETY: Forwarding Lobby/Menu speaking indicator query to vanilla EOS implementation.
        return unsafe { orig_fn(online_voice, player_state, check_range) };
    }

    let base: usize = MODULE_BASE.load(Ordering::Acquire);
    if base == 0 || online_voice.is_null() || player_state.is_null() {
        return 0;
    }

    // SAFETY: Checking whether the queried `ADH_PlayerState*` is the local player.
    let local_ps: *mut c_void = unsafe { call_get_local_dh_player_state(base, online_voice) };
    if !local_ps.is_null() && local_ps == player_state {
        // SAFETY: Local talking indicator tracks `UDH_OnlineVoice::bWantsToTalk` (`+0x80`).
        let wants: u8 = unsafe {
            *online_voice
                .byte_add(OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK)
                .cast::<u8>()
        };
        return u8::from(wants != 0);
    }

    // SAFETY: `APlayerState::UniqueId.UniqueNetId.Object` is at aligned `+0x258`.
    let unique_net_id: *const c_void = unsafe {
        *(player_state
            .byte_add(OFFSET_PS_UNIQUE_NET_ID_PTR)
            .cast::<*const c_void>())
    };
    if unique_net_id.is_null() {
        return 0;
    }

    let mut is_active: bool = false;
    // SAFETY: Converting `FUniqueNetId` to `FString` and freeing it cleanly via RAII helper.
    let _ok: bool = unsafe {
        with_unique_net_id_fstring(base, unique_net_id, |id_fstr: &FString| {
            if let Some(id_utf16) = fstring_to_utf16_vec(id_fstr) {
                is_active = is_speaker_recently_active(&id_utf16);
            }
        })
    };
    if !is_active {
        return 0;
    }

    if check_range != 0 {
        // SAFETY: Reading cached `UDH_OnlineVoice::CachedVoipMaxRange` (`+0x9C`).
        let stored_range: f32 = unsafe {
            *(online_voice
                .byte_add(OFFSET_DH_ONLINE_VOICE_MAX_RANGE)
                .cast::<f32>())
        };
        let range: f32 = if stored_range > 1.0 && stored_range.is_finite() {
            stored_range
        } else {
            FALLBACK_VOIP_MAX_RANGE
        };
        // SAFETY: Invoking Dread Hunger's native `UDH_OnlineVoice::IsInVoipRange`.
        return unsafe { call_is_in_voip_range(base, online_voice, player_state, range) };
    }

    1
}

/// Detour entry point for `UDH_OnlineVoice::IsPlayerInVoiceChat(const FString*, UObject*) -> bool`.
///
/// Keeps the Tab-menu Mute button and per-player volume slider enabled during Dedicated Server
/// matches even when EOS RTC is silenced or offline.
///
/// # Safety
/// Must only be invoked by UE4 through the verified `UDH_OnlineVoice::IsPlayerInVoiceChat` detour
/// on the game thread with valid pointers (or null).
pub unsafe extern "C" fn detour_is_player_in_voice_chat(
    player_id_str: *const FString,
    world_ctx: *mut c_void,
) -> u8 {
    if !IN_DEDICATED_MATCH.load(Ordering::Acquire) {
        let orig_addr: usize = ORIG_IS_PLAYER_IN_VOICE_CHAT.load(Ordering::Acquire);
        if orig_addr == 0 {
            return 0;
        }
        // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
        let orig_fn: IsPlayerInVoiceChatFn =
            unsafe { std::mem::transmute::<usize, IsPlayerInVoiceChatFn>(orig_addr) };
        // SAFETY: Forwarding Lobby/Menu check directly to vanilla EOS implementation.
        return unsafe { orig_fn(player_id_str, world_ctx) };
    }

    let base: usize = MODULE_BASE.load(Ordering::Acquire);
    if base == 0 || player_id_str.is_null() || world_ctx.is_null() {
        return 0;
    }

    // SAFETY: Checking whether `player_id_str` resolves to an active `ADH_PlayerState*`.
    let ps: *mut c_void = unsafe { call_get_dh_ps_from_unique_id(base, player_id_str, world_ctx) };
    u8::from(!ps.is_null())
}

unsafe fn enter_dedicated_match_mode(base: usize, voice_impl: *mut c_void, ctx: &MatchContext) {
    // SAFETY: Running on the UE4 Game Thread with verified pointers.
    unsafe {
        try_bind_eos_plus_identity(voice_impl);
        call_register_local_talker(base, voice_impl);
        if !ctx.online_voice.is_null() {
            let wants: u8 = *ctx
                .online_voice
                .byte_add(OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK)
                .cast::<u8>();
            if wants != 0 {
                call_start_networked_voice(base, voice_impl);
            } else {
                call_stop_networked_voice(base, voice_impl);
            }
            silence_eos_voice_chat_user(base, ctx.online_voice);
        }
    }
    clear_active_speakers();
    log_line("mode switch -> Dedicated Server (UE4 native 48 kHz Opus active, EOS RTC silenced)");
}

unsafe fn maintain_dedicated_match_mode(base: usize, voice_impl: *mut c_void, ctx: &MatchContext) {
    // SAFETY: Periodic game-thread maintenance while connected to a Dedicated Server.
    unsafe {
        try_bind_eos_plus_identity(voice_impl);
        if !ctx.online_voice.is_null() {
            reassert_eos_muted_if_needed(base, ctx.online_voice);
        }
    }
    prune_stale_speakers();
}

unsafe fn exit_dedicated_match_mode(base: usize, voice_impl: *mut c_void) {
    // SAFETY: Stopping native mic capture and restoring Steam identity pointer on exit.
    unsafe {
        call_stop_networked_voice(base, voice_impl);
        let orig_identity: usize = ORIG_STEAM_IDENTITY_INT.load(Ordering::Acquire);
        if orig_identity != 0 {
            *(voice_impl
                .byte_add(OFFSET_ONLINE_VOICE_IMPL_IDENTITY_INT)
                .cast::<usize>()) = orig_identity;
        }
    }
    clear_active_speakers();

    // SAFETY: Restoring EOS RTC input/output unmuted for Lobby/Menu voice.
    if let Some(online_voice) = unsafe { query_any_online_voice(base) } {
        // SAFETY: `online_voice` is a live `UDH_OnlineVoice*` from the active `UDH_GameInstance`.
        unsafe {
            restore_eos_voice_chat_user(base, online_voice);
        }
    }
    log_line("mode switch -> Lobby/Menu (UE4 native voice stopped, EOS RTC restored)");
}

unsafe fn try_bind_eos_plus_identity(voice_impl: *mut c_void) {
    let eos_plus_addr: usize = EOS_PLUS_PTR.load(Ordering::Acquire);
    if eos_plus_addr == 0 || voice_impl.is_null() {
        return;
    }

    // SAFETY: `FOnlineVoiceImpl::IdentityInt` is at aligned offset `+0x30`.
    let id_slot_ptr: *mut usize = unsafe {
        voice_impl
            .byte_add(OFFSET_ONLINE_VOICE_IMPL_IDENTITY_INT)
            .cast::<usize>()
    };
    // SAFETY: Reading current `IdentityInt` pointer.
    let current_id_int: usize = unsafe { *id_slot_ptr };
    if ORIG_STEAM_IDENTITY_INT.load(Ordering::Acquire) == 0 && current_id_int != 0 {
        ORIG_STEAM_IDENTITY_INT.store(current_id_int, Ordering::Release);
    }

    let eos_plus: *mut c_void = eos_plus_addr as *mut c_void;
    // SAFETY: `FOnlineSubsystemEOSPlus::UserManager` is verified at `+0xE8` (`0xA24450`).
    let user_mgr: *mut c_void = unsafe {
        *(eos_plus
            .byte_add(OFFSET_EOS_PLUS_USER_MANAGER)
            .cast::<*mut c_void>())
    };
    if user_mgr.is_null() {
        return;
    }

    // SAFETY: `IOnlineIdentity` subobject inside `FOnlineUserEOSPlus` is at `+0x68` (`0xA2445D`).
    let eos_identity: *mut c_void =
        unsafe { user_mgr.byte_add(OFFSET_EOS_PLUS_USER_MANAGER_IDENTITY) };
    if eos_identity as usize == current_id_int {
        return;
    }

    // Verify `eos_identity->GetUniquePlayerId(0)` produces a valid `FUniqueNetId` before swapping.
    // SAFETY: Calling `IOnlineIdentity::GetUniquePlayerId(0)` (`vtable[+0xF0]`) on `eos_identity`.
    let has_valid_id: bool = unsafe { test_identity_has_valid_local_id(eos_identity) };
    if has_valid_id {
        // SAFETY: Replacing raw non-owning `FOnlineVoiceImpl::IdentityInt` pointer so outgoing
        // voice packets carry `"SteamID|EOS_PUID"`.
        unsafe {
            *id_slot_ptr = eos_identity as usize;
        }
        log_line("bound FOnlineVoiceImpl::IdentityInt to EOSPlus identity (\"SteamID|EOS_PUID\")");
    }
}

unsafe fn test_identity_has_valid_local_id(identity: *mut c_void) -> bool {
    if identity.is_null() {
        return false;
    }
    // SAFETY: Reading vtable pointer of `IOnlineIdentity`.
    let vtable: *const usize = unsafe { *(identity.cast::<*const usize>()) };
    if vtable.is_null() {
        return false;
    }
    // SAFETY: Slot `+0xF0` is `IOnlineIdentity::GetUniquePlayerId`.
    let fn_addr: usize = unsafe { *vtable.add(VTABLE_OFF_IDENTITY_GET_UNIQUE_PLAYER_ID / 8) };
    // SAFETY: Verified `IOnlineIdentity::GetUniquePlayerId` signature.
    let get_id_fn: GetUniquePlayerIdFn =
        unsafe { std::mem::transmute::<usize, GetUniquePlayerIdFn>(fn_addr) };

    let mut out_ptr: SharedPtr = SharedPtr {
        object: std::ptr::null_mut(),
        controller: std::ptr::null_mut(),
    };
    // SAFETY: Querying local player 0 unique net ID into stack `SharedPtr`.
    let _ret: *mut SharedPtr = unsafe { get_id_fn(identity, &raw mut out_ptr, 0) };

    let valid: bool = if out_ptr.object.is_null() {
        false
    } else {
        // SAFETY: Calling `FUniqueNetId::IsValid` (`vtable[+0x28]`).
        unsafe { call_unique_net_id_is_valid(out_ptr.object) }
    };

    // SAFETY: Properly decrementing UE4 `FReferenceControllerBase` refcounts to prevent leaks.
    unsafe {
        release_shared_ptr(&mut out_ptr);
    }
    valid
}

unsafe fn release_shared_ptr(ptr: &mut SharedPtr) {
    let ctrl: *mut c_void = ptr.controller;
    ptr.object = std::ptr::null_mut();
    ptr.controller = std::ptr::null_mut();
    if ctrl.is_null() {
        return;
    }

    // SAFETY: `FReferenceControllerBase::SharedReferenceCount` is an aligned `i32` at `+0x08`.
    let shared_ref: &AtomicI32 = unsafe {
        &*(ctrl
            .byte_add(OFFSET_REF_CTRL_SHARED_COUNT)
            .cast::<AtomicI32>())
    };
    let prev_shared: i32 = shared_ref.fetch_sub(1, Ordering::AcqRel);
    if prev_shared != 1 {
        return;
    }

    // SAFETY: Reading vtable of `FReferenceControllerBase`.
    let vtable: *const usize = unsafe { *(ctrl.cast::<*const usize>()) };
    if vtable.is_null() {
        return;
    }
    // SAFETY: Calling `DestroyObject` (`vtable[+0x00]`) when shared count reaches zero.
    let destroy_addr: usize = unsafe { *vtable.add(VTABLE_OFF_REF_CTRL_DESTROY_OBJECT / 8) };
    // SAFETY: Verified `DestroyObject` signature.
    let destroy_fn: RefCtrlDestroyObjFn =
        unsafe { std::mem::transmute::<usize, RefCtrlDestroyObjFn>(destroy_addr) };
    // SAFETY: Destroying the managed object now that the last strong reference dropped.
    unsafe {
        destroy_fn(ctrl);
    }

    // SAFETY: `FReferenceControllerBase::WeakReferenceCount` is an aligned `i32` at `+0x0C`.
    let weak_ref: &AtomicI32 = unsafe {
        &*(ctrl
            .byte_add(OFFSET_REF_CTRL_WEAK_COUNT)
            .cast::<AtomicI32>())
    };
    let prev_weak: i32 = weak_ref.fetch_sub(1, Ordering::AcqRel);
    if prev_weak == 1 {
        // SAFETY: Calling deleting destructor (`vtable[+0x08]`) when weak count reaches zero.
        let dtor_addr: usize = unsafe { *vtable.add(VTABLE_OFF_REF_CTRL_DTOR / 8) };
        // SAFETY: Verified deleting destructor signature.
        let dtor_fn: RefCtrlDtorFn =
            unsafe { std::mem::transmute::<usize, RefCtrlDtorFn>(dtor_addr) };
        // SAFETY: Freeing the reference controller now that all references dropped.
        unsafe {
            dtor_fn(ctrl, 1);
        }
    }
}

unsafe fn silence_eos_voice_chat_user(base: usize, online_voice: *mut c_void) {
    // SAFETY: Querying `UDH_OnlineVoice::GetVoiceChatUser`.
    let user: *mut c_void = unsafe { call_get_voice_chat_user(base, online_voice) };
    if user.is_null() {
        return;
    }
    // SAFETY: Reading `IVoiceChatUser` vtable.
    let vtable: *const usize = unsafe { *(user.cast::<*const usize>()) };
    if vtable.is_null() {
        return;
    }
    // SAFETY: Invoking `SetAudioInputDeviceMuted(true)`, `SetAudioOutputDeviceMuted(true)`, and
    // `SetAudioInputVolume(0.0)` via verified `FEOSPlusVoiceChatUser` vtable offsets.
    unsafe {
        let set_in_muted: SetAudioMutedFn = std::mem::transmute::<usize, SetAudioMutedFn>(
            *vtable.add(VTABLE_OFF_VOICE_CHAT_SET_INPUT_MUTED / 8),
        );
        let set_out_muted: SetAudioMutedFn = std::mem::transmute::<usize, SetAudioMutedFn>(
            *vtable.add(VTABLE_OFF_VOICE_CHAT_SET_OUTPUT_MUTED / 8),
        );
        let set_in_vol: SetAudioInputVolumeFn = std::mem::transmute::<usize, SetAudioInputVolumeFn>(
            *vtable.add(VTABLE_OFF_VOICE_CHAT_SET_INPUT_VOLUME / 8),
        );
        set_in_muted(user, 1);
        set_out_muted(user, 1);
        set_in_vol(user, 0.0);
    }
}

unsafe fn reassert_eos_muted_if_needed(base: usize, online_voice: *mut c_void) {
    // SAFETY: Querying `UDH_OnlineVoice::GetVoiceChatUser`.
    let user: *mut c_void = unsafe { call_get_voice_chat_user(base, online_voice) };
    if user.is_null() {
        return;
    }
    // SAFETY: Reading `IVoiceChatUser` vtable.
    let vtable: *const usize = unsafe { *(user.cast::<*const usize>()) };
    if vtable.is_null() {
        return;
    }
    // SAFETY: Checking `GetAudioInputDeviceMuted` (`+0x48`) and `GetAudioOutputDeviceMuted` (`+0x50`).
    unsafe {
        let get_in_muted: GetAudioMutedFn = std::mem::transmute::<usize, GetAudioMutedFn>(
            *vtable.add(VTABLE_OFF_VOICE_CHAT_GET_INPUT_MUTED / 8),
        );
        let get_out_muted: GetAudioMutedFn = std::mem::transmute::<usize, GetAudioMutedFn>(
            *vtable.add(VTABLE_OFF_VOICE_CHAT_GET_OUTPUT_MUTED / 8),
        );
        if get_in_muted(user) == 0 || get_out_muted(user) == 0 {
            silence_eos_voice_chat_user(base, online_voice);
        }
    }
}

unsafe fn restore_eos_voice_chat_user(base: usize, online_voice: *mut c_void) {
    // SAFETY: Querying `UDH_OnlineVoice::GetVoiceChatUser`.
    let user: *mut c_void = unsafe { call_get_voice_chat_user(base, online_voice) };
    if !user.is_null() {
        // SAFETY: Reading `IVoiceChatUser` vtable.
        let vtable: *const usize = unsafe { *(user.cast::<*const usize>()) };
        if !vtable.is_null() {
            // SAFETY: Unmuting EOS RTC input/output devices upon returning to Lobby/Menu.
            unsafe {
                let set_in_muted: SetAudioMutedFn = std::mem::transmute::<usize, SetAudioMutedFn>(
                    *vtable.add(VTABLE_OFF_VOICE_CHAT_SET_INPUT_MUTED / 8),
                );
                let set_out_muted: SetAudioMutedFn = std::mem::transmute::<usize, SetAudioMutedFn>(
                    *vtable.add(VTABLE_OFF_VOICE_CHAT_SET_OUTPUT_MUTED / 8),
                );
                set_in_muted(user, 0);
                set_out_muted(user, 0);
            }
        }
    }

    // If Toggle-to-Talk is enabled (`bWantsToTalk != 0`), re-apply via vanilla `SetWantsToTalk`
    // so Lobby EOS voice immediately resumes transmitting without re-toggling the key.
    // SAFETY: `UDH_OnlineVoice::bWantsToTalk` is verified at `+0x80`.
    let wants: u8 = unsafe {
        *online_voice
            .byte_add(OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK)
            .cast::<u8>()
    };
    if wants != 0 {
        let orig_addr: usize = ORIG_SET_WANTS_TO_TALK.load(Ordering::Acquire);
        if orig_addr != 0 {
            // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
            let orig_fn: SetWantsToTalkFn =
                unsafe { std::mem::transmute::<usize, SetWantsToTalkFn>(orig_addr) };
            // SAFETY: Restoring vanilla EOS input volume for active Toggle-to-Talk in Lobby.
            unsafe {
                orig_fn(online_voice, 1);
            }
        }
    }
}

unsafe fn query_match_context(base: usize) -> Option<MatchContext> {
    // SAFETY: Reading active `UWorld*` from `GEngine` on the game thread.
    let world: *mut c_void = unsafe { get_game_world(base) };
    if world.is_null() {
        return None;
    }

    // SAFETY: `UWorld::NetDriver` is at aligned `+0x38`.
    let net_driver: *mut c_void = unsafe {
        *(world
            .byte_add(OFFSET_UWORLD_NET_DRIVER)
            .cast::<*mut c_void>())
    };
    if net_driver.is_null() {
        return None;
    }

    // SAFETY: `UNetDriver::ServerConnection` is at aligned `+0x88`.
    let server_conn: *mut c_void = unsafe {
        *(net_driver
            .byte_add(OFFSET_NET_DRIVER_SERVER_CONNECTION)
            .cast::<*mut c_void>())
    };
    if server_conn.is_null() {
        return None;
    }

    // SAFETY: `UWorld::GameState` is at aligned `+0x120`.
    let game_state: *mut c_void = unsafe {
        *(world
            .byte_add(OFFSET_UWORLD_GAME_STATE)
            .cast::<*mut c_void>())
    };
    if game_state.is_null() {
        return None;
    }

    // SAFETY: Verify `GameState->IsA(ADH_GameState::StaticClass())` (excludes `ADH_LobbyGameState`).
    let is_dh_game_state: bool =
        unsafe { uobject_is_a(base, game_state, RVA_ADH_GAMESTATE_STATIC_CLASS) };
    if !is_dh_game_state {
        return None;
    }

    // SAFETY: `UWorld::OwningGameInstance` is at aligned `+0x180`.
    let game_instance: *mut c_void = unsafe {
        *(world
            .byte_add(OFFSET_UWORLD_OWNING_GAME_INSTANCE)
            .cast::<*mut c_void>())
    };
    if game_instance.is_null() {
        return None;
    }
    // SAFETY: Verify `OwningGameInstance->IsA(UDH_GameInstance::StaticClass())`.
    let is_dh_gi: bool =
        unsafe { uobject_is_a(base, game_instance, RVA_UDH_GAMEINSTANCE_STATIC_CLASS) };
    if !is_dh_gi {
        return None;
    }

    // SAFETY: `UDH_GameInstance::OnlineVoice` is verified at aligned `+0x520`.
    let online_voice: *mut c_void = unsafe {
        *(game_instance
            .byte_add(OFFSET_DH_GAME_INSTANCE_ONLINE_VOICE)
            .cast::<*mut c_void>())
    };

    // SAFETY: `UGameInstance::GetFirstLocalPlayerController(gi, nullptr)` (`0x29054E0`).
    let local_controller: *mut c_void = unsafe { call_get_first_local_pc(base, game_instance) };
    let local_player_state: *mut c_void = if local_controller.is_null() {
        std::ptr::null_mut()
    } else {
        // SAFETY: `AController::PlayerState` is verified at aligned `+0x228`.
        let ps: *mut c_void = unsafe {
            *(local_controller
                .byte_add(OFFSET_CONTROLLER_PLAYER_STATE)
                .cast::<*mut c_void>())
        };
        if ps.is_null() {
            std::ptr::null_mut()
        } else {
            // SAFETY: Verify `PlayerState->IsA(ADH_PlayerState::StaticClass())`.
            let is_dh_ps: bool =
                unsafe { uobject_is_a(base, ps, RVA_ADH_PLAYERSTATE_STATIC_CLASS) };
            if is_dh_ps {
                ps
            } else {
                std::ptr::null_mut()
            }
        }
    };

    Some(MatchContext {
        world,
        game_instance,
        online_voice,
        local_pc: local_controller,
        local_ps: local_player_state,
    })
}

unsafe fn query_any_online_voice(base: usize) -> Option<*mut c_void> {
    // SAFETY: Reading active `UWorld*` from `GEngine` on the game thread.
    let world: *mut c_void = unsafe { get_game_world(base) };
    if world.is_null() {
        return None;
    }
    // SAFETY: `UWorld::OwningGameInstance` is at aligned `+0x180`.
    let game_instance: *mut c_void = unsafe {
        *(world
            .byte_add(OFFSET_UWORLD_OWNING_GAME_INSTANCE)
            .cast::<*mut c_void>())
    };
    if game_instance.is_null() {
        return None;
    }
    // SAFETY: Verify `OwningGameInstance->IsA(UDH_GameInstance::StaticClass())`.
    let is_dh_gi: bool =
        unsafe { uobject_is_a(base, game_instance, RVA_UDH_GAMEINSTANCE_STATIC_CLASS) };
    if !is_dh_gi {
        return None;
    }
    // SAFETY: `UDH_GameInstance::OnlineVoice` is at aligned `+0x520`.
    let online_voice: *mut c_void = unsafe {
        *(game_instance
            .byte_add(OFFSET_DH_GAME_INSTANCE_ONLINE_VOICE)
            .cast::<*mut c_void>())
    };
    if online_voice.is_null() {
        None
    } else {
        Some(online_voice)
    }
}

unsafe fn get_game_world(base: usize) -> *mut c_void {
    let Some(gengine_addr): Option<usize> = base.checked_add(RVA_GENGINE) else {
        return std::ptr::null_mut();
    };
    // SAFETY: `GEngine` is a global pointer inside the main client image.
    let engine: *mut c_void = unsafe { *(gengine_addr as *const *mut c_void) };
    if engine.is_null() {
        return std::ptr::null_mut();
    }
    let Some(fn_addr): Option<usize> = base.checked_add(RVA_UGAMEENGINE_GET_GAME_WORLD) else {
        return std::ptr::null_mut();
    };
    // SAFETY: Verified `UGameEngine::GetGameWorld` address and signature.
    let get_world_fn: GetGameWorldFn =
        unsafe { std::mem::transmute::<usize, GetGameWorldFn>(fn_addr) };
    // SAFETY: Querying the active game `UWorld*` from `GEngine` on the game thread.
    unsafe { get_world_fn(engine) }
}

unsafe fn uobject_is_a(base: usize, obj: *const c_void, static_class_rva: usize) -> bool {
    if obj.is_null() {
        return false;
    }
    let Some(cls_fn_addr): Option<usize> = base.checked_add(static_class_rva) else {
        return false;
    };
    // SAFETY: Calling UE4 `T::GetPrivateStaticClass()`.
    let get_cls_fn: GetPrivateStaticClassFn =
        unsafe { std::mem::transmute::<usize, GetPrivateStaticClassFn>(cls_fn_addr) };
    // SAFETY: Returns singleton `UClass*` for the requested class.
    let target_cls: *mut c_void = unsafe { get_cls_fn() };
    if target_cls.is_null() {
        return false;
    }

    // SAFETY: Reading `UObjectBase::ClassPrivate` (`+0x10`).
    let obj_cls: *const c_void =
        unsafe { *(obj.byte_add(OFFSET_UOBJECT_CLASS).cast::<*const c_void>()) };
    if obj_cls.is_null() {
        return false;
    }

    // SAFETY: `FStructBaseChain` depth (`i32` at aligned `+0x38`).
    let target_depth: i32 = unsafe {
        *(target_cls
            .byte_add(OFFSET_UCLASS_BASE_CHAIN_DEPTH)
            .cast::<i32>())
    };
    // SAFETY: Reading `obj_cls` base chain depth (`+0x38`).
    let obj_depth: i32 = unsafe {
        *(obj_cls
            .byte_add(OFFSET_UCLASS_BASE_CHAIN_DEPTH)
            .cast::<i32>())
    };
    if target_depth < 0 || target_depth > obj_depth {
        return false;
    }
    let Ok(depth_idx): Result<usize, std::num::TryFromIntError> = usize::try_from(target_depth)
    else {
        return false;
    };
    // SAFETY: Reading `obj_cls->StructBaseChainArray` (`+0x30`).
    let chain_arr: *const usize = unsafe {
        *(obj_cls
            .byte_add(OFFSET_UCLASS_BASE_CHAIN_ARRAY)
            .cast::<*const usize>())
    };
    if chain_arr.is_null() {
        return false;
    }
    // SAFETY: `depth_idx <= obj_depth`, so `chain_arr.add(depth_idx)` is in bounds.
    let chain_entry: usize = unsafe { *chain_arr.add(depth_idx) };
    let expected_chain_ptr: usize =
        (target_cls as usize).wrapping_add(OFFSET_UCLASS_BASE_CHAIN_ARRAY);
    chain_entry == expected_chain_ptr
}

unsafe fn ensure_voice_channel_definition(net_driver: *mut c_void) {
    let base: usize = MODULE_BASE.load(Ordering::Acquire);
    if base == 0 {
        return;
    }

    // SAFETY: `UObjectBase::ObjectFlags` lives at aligned `+0x08` on every `UObject`.
    let obj_flags: u32 = unsafe { *(net_driver.byte_add(OFFSET_UOBJECT_FLAGS).cast::<u32>()) };
    if (obj_flags & UOBJECT_FLAG_CDO) != 0 {
        return;
    }

    let Some(voice_name_id): Option<u32> = resolve_voice_ename_id(base) else {
        log_line("LoadChannelDefinitions: failed to resolve ENAME_VOICE");
        return;
    };

    // SAFETY: `UNetDriver::ChannelDefinitions` (`TArray<FChannelDefinition>`) is at aligned `+0x198`.
    let arr_ptr: *mut TArrayRaw = unsafe {
        net_driver
            .byte_add(OFFSET_NET_DRIVER_CHANNEL_DEFINITIONS)
            .cast::<TArrayRaw>()
    };
    // SAFETY: `arr_ptr` points inside the live `UNetDriver`.
    let arr_snapshot: TArrayRaw = unsafe { *arr_ptr };
    if arr_snapshot.num < 0
        || arr_snapshot.num > MAX_SANE_CHANNEL_DEFINITIONS
        || arr_snapshot.max < arr_snapshot.num
        || (arr_snapshot.num > 0 && arr_snapshot.data.is_null())
    {
        return;
    }

    let Ok(count): Result<usize, std::num::TryFromIntError> = usize::try_from(arr_snapshot.num)
    else {
        return;
    };

    let mut static_one_taken: bool = false;
    if count > 0 {
        // SAFETY: `arr_snapshot.data` holds at least `count` contiguous `ChannelDefinition` items.
        let entries: &mut [ChannelDefinition] = unsafe {
            std::slice::from_raw_parts_mut(arr_snapshot.data.cast::<ChannelDefinition>(), count)
        };
        for entry in entries {
            if entry.static_channel_index == VOICE_DEFAULT_STATIC_CHANNEL_INDEX {
                static_one_taken = true;
            }
            if entry.channel_name.comparison_index == voice_name_id
                && entry.channel_name.number == 0
            {
                if entry.class_name.comparison_index == 0 {
                    entry.class_name = make_voice_class_fname(base);
                }
                entry.b_tick_on_create = 1;
                entry.b_server_open = 1;
                entry.b_client_open = 1;
                entry.b_initial_server = 1;
                entry.b_initial_client = 1;
                log_line(&format!(
                    "LoadChannelDefinitions: verified existing Voice channel (static_idx={}, total={count})",
                    entry.static_channel_index
                ));
                return;
            }
        }
    }

    let mut data_ptr: *mut ChannelDefinition = arr_snapshot.data.cast::<ChannelDefinition>();
    let mut new_max: i32 = arr_snapshot.max;
    if arr_snapshot.num >= arr_snapshot.max {
        let desired_max: i32 = arr_snapshot
            .max
            .max(MIN_CHANNEL_DEFINITIONS_CAPACITY)
            .saturating_mul(2);
        let Ok(desired_max_usize): Result<usize, std::num::TryFromIntError> =
            usize::try_from(desired_max)
        else {
            return;
        };
        let Some(byte_size): Option<usize> =
            desired_max_usize.checked_mul(std::mem::size_of::<ChannelDefinition>())
        else {
            return;
        };
        let Some(realloc_addr): Option<usize> = base.checked_add(RVA_FMEMORY_REALLOC) else {
            return;
        };
        // SAFETY: `RVA_FMEMORY_REALLOC` is verified `FMemory::Realloc` in client image.
        let realloc_fn: FMemoryReallocFn =
            unsafe { std::mem::transmute::<usize, FMemoryReallocFn>(realloc_addr) };
        // SAFETY: Growing UE4 `TArray` storage using UE4's own `FMemory::Realloc`.
        let grown: *mut c_void = unsafe { realloc_fn(arr_snapshot.data, byte_size, 0) };
        if grown.is_null() {
            log_line("LoadChannelDefinitions: FMemory::Realloc failed while appending Voice");
            return;
        }
        data_ptr = grown.cast::<ChannelDefinition>();
        new_max = desired_max;
        // SAFETY: Updating `TArrayRaw` buffer pointer and capacity in-place.
        unsafe {
            (*arr_ptr).data = grown;
            (*arr_ptr).max = new_max;
        }
    }

    let static_channel_index: i32 = if static_one_taken {
        DYNAMIC_STATIC_CHANNEL_INDEX
    } else {
        VOICE_DEFAULT_STATIC_CHANNEL_INDEX
    };
    let voice_def: ChannelDefinition = ChannelDefinition {
        channel_name: FName {
            comparison_index: voice_name_id,
            number: 0,
        },
        class_name: make_voice_class_fname(base),
        channel_class: std::ptr::null_mut(),
        static_channel_index,
        b_tick_on_create: 1,
        b_server_open: 1,
        b_client_open: 1,
        b_initial_server: 1,
        b_initial_client: 1,
        pad: [0u8; 7],
    };

    // SAFETY: `count < new_max`, so `data_ptr.add(count)` is a valid writable slot.
    unsafe {
        data_ptr.add(count).write(voice_def);
        (*arr_ptr).num = arr_snapshot.num + 1;
    }

    log_line(&format!(
        "LoadChannelDefinitions: appended Voice channel (static_idx={static_channel_index}, new_num={}, max={new_max})",
        arr_snapshot.num + 1
    ));
}

fn resolve_voice_ename_id(module_base: usize) -> Option<u32> {
    let fn_addr: usize = module_base.checked_add(RVA_FNAME_ENTRY_ID_FROM_VALID_ENAME)?;
    // SAFETY: `RVA_FNAME_ENTRY_ID_FROM_VALID_ENAME` is verified `FNameEntryId::FromValidEName`.
    let from_valid_ename: FNameEntryIdFromValidENameFn =
        unsafe { std::mem::transmute::<usize, FNameEntryIdFromValidENameFn>(fn_addr) };
    let mut entry_id: u32 = 0;
    // SAFETY: Passing valid stack output pointer and built-in `ENAME_VOICE` (`0x100`).
    let _out: *mut u32 = unsafe { from_valid_ename(&raw mut entry_id, ENAME_VOICE) };
    if entry_id == 0 {
        None
    } else {
        Some(entry_id)
    }
}

fn make_voice_class_fname(module_base: usize) -> FName {
    let mut out_name: FName = FName {
        comparison_index: 0,
        number: 0,
    };
    let Some(ctor_addr): Option<usize> = module_base.checked_add(RVA_FNAME_CTOR_ANSI) else {
        return out_name;
    };
    // SAFETY: `RVA_FNAME_CTOR_ANSI` is verified `FName::FName(const ANSICHAR*, EFindName)`.
    let ctor_fn: FNameCtorAnsiFn =
        unsafe { std::mem::transmute::<usize, FNameCtorAnsiFn>(ctor_addr) };
    // SAFETY: `out_name` is a valid writable `FName` and `VOICE_CHANNEL_CLASS_PATH` is a static CStr.
    let _res: *mut FName = unsafe {
        ctor_fn(
            &raw mut out_name,
            VOICE_CHANNEL_CLASS_PATH.as_ptr(),
            FNAME_ADD,
        )
    };
    out_name
}

unsafe fn call_unique_net_id_is_valid(unique_id: *const c_void) -> bool {
    if unique_id.is_null() {
        return false;
    }
    // SAFETY: Reading `FUniqueNetId` vtable.
    let vtable: *const usize = unsafe { *(unique_id.cast::<*const usize>()) };
    if vtable.is_null() {
        return false;
    }
    // SAFETY: Slot `+0x28` is `FUniqueNetId::IsValid()`.
    let fn_addr: usize = unsafe { *vtable.add(VTABLE_OFF_UNIQUE_NET_ID_IS_VALID / 8) };
    // SAFETY: Verified `FUniqueNetId::IsValid` signature.
    let is_valid_fn: UniqueNetIdIsValidFn =
        unsafe { std::mem::transmute::<usize, UniqueNetIdIsValidFn>(fn_addr) };
    // SAFETY: Calling `IsValid()` on live `FUniqueNetId`.
    unsafe { is_valid_fn(unique_id) != 0 }
}

unsafe fn with_unique_net_id_fstring<F>(base: usize, unique_id: *const c_void, func: F) -> bool
where
    F: FnOnce(&FString),
{
    if unique_id.is_null() {
        return false;
    }
    // SAFETY: `unique_id` is a non-null `const FUniqueNetId*` verified by caller.
    let is_valid: bool = unsafe { call_unique_net_id_is_valid(unique_id) };
    if !is_valid {
        return false;
    }
    // SAFETY: Reading `FUniqueNetId` vtable.
    let vtable: *const usize = unsafe { *(unique_id.cast::<*const usize>()) };
    if vtable.is_null() {
        return false;
    }
    // SAFETY: Slot `+0x30` is `FUniqueNetId::ToString(FString* out)`.
    let to_str_addr: usize = unsafe { *vtable.add(VTABLE_OFF_UNIQUE_NET_ID_TO_STRING / 8) };
    // SAFETY: Verified `FUniqueNetId::ToString` signature.
    let to_str_fn: UniqueNetIdToStringFn =
        unsafe { std::mem::transmute::<usize, UniqueNetIdToStringFn>(to_str_addr) };

    let mut fstr: FString = FString {
        data: std::ptr::null_mut(),
        num: 0,
        max: 0,
    };
    // SAFETY: Populates `fstr` using UE4's `FMemory` allocator.
    let _out: *mut FString = unsafe { to_str_fn(unique_id, &raw mut fstr) };

    let valid: bool = !fstr.data.is_null() && fstr.num > 1;
    if valid {
        func(&fstr);
    }

    if !fstr.data.is_null() {
        if let Some(free_addr) = base.checked_add(RVA_FMEMORY_FREE) {
            // SAFETY: Verified `FMemory::Free` signature.
            let free_fn: FMemoryFreeFn =
                unsafe { std::mem::transmute::<usize, FMemoryFreeFn>(free_addr) };
            // SAFETY: Freeing UE4-allocated `FString::data` buffer exactly once.
            unsafe {
                free_fn(fstr.data.cast::<c_void>());
            }
        }
    }
    valid
}

unsafe fn fstring_to_utf16_vec(fstr: &FString) -> Option<Vec<u16>> {
    if fstr.data.is_null() || fstr.num <= 1 {
        return None;
    }
    let char_len: usize = usize::try_from(fstr.num - 1).ok()?;
    if char_len == 0 || char_len > 256 {
        return None;
    }
    // SAFETY: `fstr.data` holds `fstr.num` UTF-16 code units (last unit is trailing NUL).
    let slice: &[u16] = unsafe { std::slice::from_raw_parts(fstr.data, char_len) };
    Some(slice.to_vec())
}

fn has_audible_speech(samples: &[i16]) -> bool {
    for sample in samples {
        let abs_val: i32 = i32::from(*sample).abs();
        if abs_val >= i32::from(SPEECH_PEAK_THRESHOLD) {
            return true;
        }
    }
    false
}

fn record_active_speaker(id_utf16: Vec<u16>) {
    let Ok(mut speakers): Result<std::sync::MutexGuard<'_, Vec<SpeakerEntry>>, _> =
        ACTIVE_SPEAKERS.lock()
    else {
        return;
    };
    let now: Instant = Instant::now();
    for entry in speakers.iter_mut() {
        if entry.id_utf16 == id_utf16 {
            entry.last_heard = now;
            return;
        }
    }
    if speakers.len() >= MAX_TRACKED_SPEAKERS {
        let cutoff: Duration = Duration::from_millis(SPEAKER_PRUNE_WINDOW_MS);
        speakers.retain(|entry: &SpeakerEntry| now.duration_since(entry.last_heard) <= cutoff);
    }
    if speakers.len() < MAX_TRACKED_SPEAKERS {
        speakers.push(SpeakerEntry {
            id_utf16,
            last_heard: now,
        });
    }
}

fn is_speaker_recently_active(id_utf16: &[u16]) -> bool {
    let Ok(speakers): Result<std::sync::MutexGuard<'_, Vec<SpeakerEntry>>, _> =
        ACTIVE_SPEAKERS.lock()
    else {
        return false;
    };
    let now: Instant = Instant::now();
    let window: Duration = Duration::from_millis(SPEAKER_ACTIVE_WINDOW_MS);
    for entry in speakers.iter() {
        if entry.id_utf16.as_slice() == id_utf16 {
            return now.duration_since(entry.last_heard) <= window;
        }
    }
    false
}

fn prune_stale_speakers() {
    let Ok(mut speakers): Result<std::sync::MutexGuard<'_, Vec<SpeakerEntry>>, _> =
        ACTIVE_SPEAKERS.lock()
    else {
        return;
    };
    let now: Instant = Instant::now();
    let cutoff: Duration = Duration::from_millis(SPEAKER_PRUNE_WINDOW_MS);
    speakers.retain(|entry: &SpeakerEntry| now.duration_since(entry.last_heard) <= cutoff);
}

fn clear_active_speakers() {
    if let Ok(mut speakers) = ACTIVE_SPEAKERS.lock() {
        speakers.clear();
    }
}

unsafe fn call_get_first_local_pc(base: usize, game_instance: *mut c_void) -> *mut c_void {
    let Some(addr): Option<usize> = base.checked_add(RVA_UGAMEINSTANCE_GET_FIRST_LOCAL_PC) else {
        return std::ptr::null_mut();
    };
    // SAFETY: Verified `UGameInstance::GetFirstLocalPlayerController` address & signature.
    let func: GetFirstLocalPcFn = unsafe { std::mem::transmute::<usize, GetFirstLocalPcFn>(addr) };
    // SAFETY: Passing live `UGameInstance*` and null world filter (identical to `GetLocalDHPlayerState`).
    unsafe { func(game_instance, std::ptr::null_mut()) }
}

unsafe fn call_register_local_talker(base: usize, voice_impl: *mut c_void) {
    let Some(addr): Option<usize> = base.checked_add(RVA_ONLINE_VOICE_REGISTER_LOCAL_TALKER) else {
        return;
    };
    // SAFETY: Verified `FOnlineVoiceImpl::RegisterLocalTalker` address & signature.
    let func: RegisterLocalTalkerFn =
        unsafe { std::mem::transmute::<usize, RegisterLocalTalkerFn>(addr) };
    // SAFETY: Registering local player `0` (idempotent when already registered).
    let _ok: u8 = unsafe { func(voice_impl, 0) };
}

unsafe fn call_start_networked_voice(base: usize, voice_impl: *mut c_void) {
    let Some(addr): Option<usize> = base.checked_add(RVA_ONLINE_VOICE_START_NETWORKED_VOICE) else {
        return;
    };
    // SAFETY: Verified `FOnlineVoiceImpl::StartNetworkedVoice` address & signature.
    let func: StartStopNetworkedVoiceFn =
        unsafe { std::mem::transmute::<usize, StartStopNetworkedVoiceFn>(addr) };
    // SAFETY: Starting local player `0` microphone capture on the game thread.
    unsafe {
        func(voice_impl, 0);
    }
}

unsafe fn call_stop_networked_voice(base: usize, voice_impl: *mut c_void) {
    let Some(addr): Option<usize> = base.checked_add(RVA_ONLINE_VOICE_STOP_NETWORKED_VOICE) else {
        return;
    };
    // SAFETY: Verified `FOnlineVoiceImpl::StopNetworkedVoice` address & signature.
    let func: StartStopNetworkedVoiceFn =
        unsafe { std::mem::transmute::<usize, StartStopNetworkedVoiceFn>(addr) };
    // SAFETY: Stopping local player `0` microphone capture on the game thread.
    unsafe {
        func(voice_impl, 0);
    }
}

unsafe fn call_get_voice_chat_user(base: usize, online_voice: *mut c_void) -> *mut c_void {
    let Some(addr): Option<usize> = base.checked_add(RVA_DH_ONLINE_VOICE_GET_VOICE_CHAT_USER)
    else {
        return std::ptr::null_mut();
    };
    // SAFETY: Verified `UDH_OnlineVoice::GetVoiceChatUser` address & signature.
    let func: GetVoiceChatUserFn =
        unsafe { std::mem::transmute::<usize, GetVoiceChatUserFn>(addr) };
    // SAFETY: Invoking `GetVoiceChatUser` on live `UDH_OnlineVoice*`.
    unsafe { func(online_voice) }
}

unsafe fn call_before_recv_audio_rendered(
    base: usize,
    online_voice_slot: *const *mut c_void,
    sender_id_str: *const FString,
    pcm_samples: *const TArrayViewI16,
) {
    let Some(addr): Option<usize> = base.checked_add(RVA_DH_ONLINE_VOICE_BEFORE_RECV_AUDIO) else {
        return;
    };
    // SAFETY: Verified `UDH_OnlineVoice::BeforeRecvAudioRendered` address & signature (`rcx = UDH_OnlineVoice**`).
    let func: BeforeRecvAudioRenderedFn =
        unsafe { std::mem::transmute::<usize, BeforeRecvAudioRenderedFn>(addr) };
    // SAFETY: Feeding 48 kHz mono `i16` PCM directly into Dread Hunger's 3D/2D spatial audio pipeline.
    unsafe {
        func(online_voice_slot, sender_id_str, pcm_samples);
    }
}

unsafe fn call_get_dh_ps_from_unique_id(
    base: usize,
    unique_id_str: *const FString,
    world_ctx: *mut c_void,
) -> *mut c_void {
    let Some(addr): Option<usize> = base.checked_add(RVA_DH_GET_PS_FROM_UNIQUE_ID_STR) else {
        return std::ptr::null_mut();
    };
    // SAFETY: Verified `UDH_GameplayStatics::GetDHPlayerStateFromStringUniqueId` address & signature.
    let func: GetDhPlayerStateFromUniqueIdFn =
        unsafe { std::mem::transmute::<usize, GetDhPlayerStateFromUniqueIdFn>(addr) };
    // SAFETY: Resolving `ADH_PlayerState*` from `"SteamID|EOS_PUID"` string on game thread.
    unsafe { func(unique_id_str, world_ctx) }
}

unsafe fn call_get_local_dh_player_state(base: usize, world_ctx: *mut c_void) -> *mut c_void {
    let Some(addr): Option<usize> = base.checked_add(RVA_DH_GET_LOCAL_PLAYER_STATE) else {
        return std::ptr::null_mut();
    };
    // SAFETY: Verified `UDH_GameplayStatics::GetLocalDHPlayerState` address & signature.
    let func: GetLocalDhPlayerStateFn =
        unsafe { std::mem::transmute::<usize, GetLocalDhPlayerStateFn>(addr) };
    // SAFETY: Querying local `ADH_PlayerState*` on game thread.
    unsafe { func(world_ctx) }
}

unsafe fn call_is_in_voip_range(
    base: usize,
    online_voice: *mut c_void,
    player_state: *mut c_void,
    max_range: f32,
) -> u8 {
    let Some(addr): Option<usize> = base.checked_add(RVA_DH_ONLINE_VOICE_IS_IN_VOIP_RANGE) else {
        return 1;
    };
    // SAFETY: Verified `UDH_OnlineVoice::IsInVoipRange` address & signature.
    let func: IsInVoipRangeFn = unsafe { std::mem::transmute::<usize, IsInVoipRangeFn>(addr) };
    // SAFETY: Querying native 3D VOIP hearing range for speaking indicator UI.
    unsafe { func(online_voice, player_state, max_range) }
}
