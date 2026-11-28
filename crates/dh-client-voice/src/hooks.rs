use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use dh_voice_proto::is_dhv2_packet;

use crate::consts::dsp::{
    OPUS_FRAME_SAMPLES_20MS, REMOTE_ENERGY_ATTACK_ALPHA, REMOTE_ENERGY_RELEASE_ALPHA,
};
use crate::consts::engine::{
    FSOCKET_TYPE_DATAGRAM, MAX_DHV2_DRAIN_PER_RECV, MAX_UE4_UID_UTF16_LEN, SOCKET_RECV_FLAG_PEEK,
};
use crate::consts::ue4::{
    FALLBACK_VOIP_MAX_RANGE, MATCH_REASSERT_TICK_INTERVAL, MAX_SANE_DELEGATE_LISTENERS,
    MAX_TRACKED_SPEAKERS, OFFSET_CONTROLLER_PLAYER_STATE, OFFSET_DH_GAME_INSTANCE_MESSAGE_BUS,
    OFFSET_DH_GAME_INSTANCE_ONLINE_VOICE, OFFSET_DH_MESSAGE_BUS_ON_VOICE_CHAT_PARTICIPANT_CHANGED,
    OFFSET_DH_ONLINE_VOICE_MAX_RANGE, OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK,
    OFFSET_FSOCKET_BSD_RAW_SOCKET, OFFSET_FSOCKET_BSD_SOCKET_TYPE, OFFSET_MULTICAST_DELEGATE_NUM,
    OFFSET_NET_DRIVER_SERVER_CONNECTION, OFFSET_PS_CAN_HEAR_GHOSTS, OFFSET_PS_IS_GHOST,
    OFFSET_PS_IS_MUTED, OFFSET_PS_UNIQUE_NET_ID_PTR, OFFSET_UCLASS_BASE_CHAIN_ARRAY,
    OFFSET_UCLASS_BASE_CHAIN_DEPTH, OFFSET_UOBJECT_CLASS, OFFSET_UWORLD_GAME_STATE,
    OFFSET_UWORLD_NET_DRIVER, OFFSET_UWORLD_OWNING_GAME_INSTANCE, RVA_ADH_GAMESTATE_STATIC_CLASS,
    RVA_ADH_PLAYERSTATE_STATIC_CLASS, RVA_DH_GET_LOCAL_PLAYER_STATE,
    RVA_DH_GET_PS_FROM_UNIQUE_ID_STR, RVA_DH_ONLINE_VOICE_BEFORE_RECV_AUDIO,
    RVA_DH_ONLINE_VOICE_GET_VOICE_CHAT_USER, RVA_DH_ONLINE_VOICE_IS_IN_VOIP_RANGE,
    RVA_FMEMORY_FREE, RVA_GENGINE, RVA_PROCESS_MULTICAST_DELEGATE,
    RVA_UDH_GAMEINSTANCE_STATIC_CLASS, RVA_UGAMEENGINE_GET_GAME_WORLD,
    RVA_UGAMEINSTANCE_GET_FIRST_LOCAL_PC, RVA_UIPNETDRIVER_GET_SOCKET,
    RVA_UIPNETDRIVER_STATIC_CLASS, SPEAKER_ACTIVE_WINDOW_MS, SPEAKER_PRUNE_WINDOW_MS,
    SPEAKER_UI_REFRESH_INTERVAL_MS, SPEECH_PEAK_THRESHOLD, VTABLE_OFF_UNIQUE_NET_ID_IS_VALID,
    VTABLE_OFF_UNIQUE_NET_ID_TO_STRING, VTABLE_OFF_VOICE_CHAT_GET_INPUT_MUTED,
    VTABLE_OFF_VOICE_CHAT_GET_OUTPUT_MUTED, VTABLE_OFF_VOICE_CHAT_SET_INPUT_MUTED,
    VTABLE_OFF_VOICE_CHAT_SET_INPUT_VOLUME, VTABLE_OFF_VOICE_CHAT_SET_OUTPUT_MUTED,
};
use crate::dsp::{decay_meter_energy, pcm_i16_rms_energy, smooth_meter_energy};
use crate::engine::{
    drain_ready_playout_frames, on_dhv2_datagram_received, on_socket_close_observed,
    on_socket_send_to_observed, read_local_mic_energy, reset_local_mic_energy,
    set_local_wants_to_talk, set_match_game_socket_ptr, start_match_session, stop_match_session,
    tick_hello_keepalive,
};
use crate::logger::log_line;
use crate::structs::engine::DecodedPlayoutFrame;
use crate::structs::opus::OpusApiTable;
use crate::structs::state::{MatchContext, SpeakerEntry};
use crate::structs::ue4::{
    BeforeRecvAudioRenderedFn, EngineLoopTickFn, FMemoryFreeFn, FString, GetAudioMutedFn,
    GetDhPlayerStateFromUniqueIdFn, GetFirstLocalPcFn, GetGameWorldFn, GetLocalDhPlayerStateFn,
    GetNetDriverSocketFn, GetParticipantAudioEnergyFn, GetPrivateStaticClassFn, GetVoiceChatUserFn,
    GetWantsToTalkFn, IsInVoipRangeFn, IsParticipantSpeakingFn, IsPlayerInVoiceChatFn,
    ProcessMulticastDelegateFn, SetAudioInputVolumeFn, SetAudioMutedFn, SetWantsToTalkFn,
    SocketCloseFn, SocketRecvFromFn, SocketSendToFn, TArrayViewI16, UniqueNetIdIsValidFn,
    UniqueNetIdToStringFn,
};

static MODULE_BASE: AtomicUsize = AtomicUsize::new(0);
static MODULE_IMAGE_SIZE: AtomicUsize = AtomicUsize::new(0);

/// Single-byte flag read directly by the naked `FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered`
/// guard stub (`1` while connected to a Dedicated Server match, `0` in Main Menu or Lobby).
pub static IN_DEDICATED_MATCH: AtomicBool = AtomicBool::new(false);

static MATCH_TICK_COUNTER: AtomicU32 = AtomicU32::new(0);

static ORIG_SOCKET_RECV_FROM: AtomicUsize = AtomicUsize::new(0);
static ORIG_SOCKET_SEND_TO: AtomicUsize = AtomicUsize::new(0);
static ORIG_SOCKET_CLOSE: AtomicUsize = AtomicUsize::new(0);
static ORIG_ENGINE_LOOP_TICK: AtomicUsize = AtomicUsize::new(0);
static ORIG_SET_WANTS_TO_TALK: AtomicUsize = AtomicUsize::new(0);
static ORIG_GET_WANTS_TO_TALK: AtomicUsize = AtomicUsize::new(0);
static ORIG_IS_PARTICIPANT_SPEAKING: AtomicUsize = AtomicUsize::new(0);
static ORIG_IS_PLAYER_IN_VOICE_CHAT: AtomicUsize = AtomicUsize::new(0);
static ORIG_GET_PARTICIPANT_AUDIO_ENERGY: AtomicUsize = AtomicUsize::new(0);

static ACTIVE_SPEAKERS: Mutex<Vec<SpeakerEntry>> = Mutex::new(Vec::new());
static RESOLVED_OPUS_API: OnceLock<OpusApiTable> = OnceLock::new();

/// Stores the verified `DreadHunger-Win64-Shipping.exe` base address and PE `SizeOfImage`
/// before installing detours.
pub fn set_module_image(module_base: usize, image_size: usize) {
    MODULE_BASE.store(module_base, Ordering::Release);
    MODULE_IMAGE_SIZE.store(image_size, Ordering::Release);
}

/// Registers the original-call trampoline address for `FSocketBSD::RecvFrom`.
pub fn set_orig_socket_recv_from(addr: usize) {
    ORIG_SOCKET_RECV_FROM.store(addr, Ordering::Release);
}

/// Registers the original-call trampoline address for `FSocketBSD::SendTo`.
pub fn set_orig_socket_send_to(addr: usize) {
    ORIG_SOCKET_SEND_TO.store(addr, Ordering::Release);
}

/// Registers the original-call trampoline address for `FSocketBSD::Close`.
pub fn set_orig_socket_close(addr: usize) {
    ORIG_SOCKET_CLOSE.store(addr, Ordering::Release);
}

/// Registers the original-call trampoline address for `FEngineLoop::Tick`.
pub fn set_orig_engine_loop_tick(addr: usize) {
    ORIG_ENGINE_LOOP_TICK.store(addr, Ordering::Release);
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

/// Registers the original-call trampoline address for
/// `UDH_GameplayStatics::GetParticipantPositionalAudioEnergy`.
pub fn set_orig_get_participant_audio_energy(addr: usize) {
    ORIG_GET_PARTICIPANT_AUDIO_ENERGY.store(addr, Ordering::Release);
}

/// Detour entry point for `FSocketBSD::RecvFrom(this, Data, BufSize, BytesRead, Source, Flags)`.
///
/// Drains up to `128` `DHV2` voice datagrams on UDP sockets before returning control to UE4
/// `UNetConnection`, including consuming `MSG_PEEK` (`flags & 2`) datagrams so UE4 never loops.
///
/// # Safety
/// Must only be invoked by UE4 through our verified `FSocketBSD::RecvFrom` detour with valid
/// pointer arguments matching UE4's `FSocketBSD::RecvFrom` ABI.
pub unsafe extern "C" fn detour_socket_recv_from(
    socket: *mut c_void,
    data: *mut u8,
    buffer_size: i32,
    bytes_read: *mut i32,
    source: *mut c_void,
    flags: u32,
) -> u8 {
    let orig_addr: usize = ORIG_SOCKET_RECV_FROM.load(Ordering::Acquire);
    if orig_addr == 0 {
        return 0;
    }
    // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
    let orig_fn: SocketRecvFromFn =
        unsafe { std::mem::transmute::<usize, SocketRecvFromFn>(orig_addr) };

    if socket.is_null() || data.is_null() || bytes_read.is_null() || buffer_size <= 0 {
        // SAFETY: Forwarding invalid/edge arguments directly to original `FSocketBSD::RecvFrom`.
        return unsafe { orig_fn(socket, data, buffer_size, bytes_read, source, flags) };
    }

    // SAFETY: `FSocketBSD::SocketType` (`u32`) is verified at `+0x08`.
    let socket_type: u32 = unsafe {
        *(socket
            .byte_add(OFFSET_FSOCKET_BSD_SOCKET_TYPE)
            .cast::<u32>())
    };
    if socket_type != FSOCKET_TYPE_DATAGRAM {
        // SAFETY: Non-UDP (e.g., TCP) sockets pass straight through untouched.
        return unsafe { orig_fn(socket, data, buffer_size, bytes_read, source, flags) };
    }

    let max_buf_usize: usize = usize::try_from(buffer_size).unwrap_or(0);
    for _iter in 0..MAX_DHV2_DRAIN_PER_RECV {
        // SAFETY: Invoking original `FSocketBSD::RecvFrom` on live UDP socket.
        let ok: u8 = unsafe { orig_fn(socket, data, buffer_size, bytes_read, source, flags) };
        if ok == 0 {
            return 0;
        }
        // SAFETY: `bytes_read` is non-null caller-supplied output pointer.
        let read_i32: i32 = unsafe { *bytes_read };
        if read_i32 <= 0 {
            return ok;
        }
        let read_usize: usize = usize::try_from(read_i32).unwrap_or(0).min(max_buf_usize);
        // SAFETY: `data` holds `read_usize` readable bytes just populated by `orig_fn`.
        let packet_slice: &[u8] = unsafe { std::slice::from_raw_parts(data, read_usize) };
        if !is_dhv2_packet(packet_slice) {
            return ok;
        }

        // If UE4 peeked (`MSG_PEEK`, bit 1), consume the `DHV2` datagram from the OS queue so
        // UE4 does not repeatedly peek the same voice packet.
        let effective_len: usize = if (flags & SOCKET_RECV_FLAG_PEEK) != 0 {
            // SAFETY: Consuming the peeked `DHV2` datagram with `MSG_PEEK` cleared.
            let consume_ok: u8 = unsafe {
                orig_fn(
                    socket,
                    data,
                    buffer_size,
                    bytes_read,
                    source,
                    flags & !SOCKET_RECV_FLAG_PEEK,
                )
            };
            if consume_ok == 0 {
                read_usize
            } else {
                // SAFETY: `bytes_read` holds the consumed byte count.
                let consumed_i32: i32 = unsafe { *bytes_read };
                usize::try_from(consumed_i32)
                    .unwrap_or(0)
                    .min(max_buf_usize)
            }
        } else {
            read_usize
        };

        // SAFETY: `FSocketBSD::Socket` (Win32 `SOCKET`) is verified at `+0x28`.
        let raw_socket: usize = unsafe {
            *(socket
                .byte_add(OFFSET_FSOCKET_BSD_RAW_SOCKET)
                .cast::<usize>())
        };
        // SAFETY: `data` holds `effective_len` readable bytes of the `DHV2` packet.
        let dhv2_slice: &[u8] = unsafe { std::slice::from_raw_parts(data, effective_len) };
        // SAFETY: `source` was populated by `FSocketBSD::RecvFrom` as a live `FInternetAddrBSD`.
        unsafe {
            on_dhv2_datagram_received(socket as usize, raw_socket, source, dhv2_slice);
        }
    }

    // Flood guard: emulate `WSAEWOULDBLOCK` (`*bytes_read = 0`, return `true`) so UE4 never sees
    // leftover `DHV2` bytes in `data`.
    // SAFETY: `bytes_read` is non-null caller-supplied output pointer.
    unsafe {
        *bytes_read = 0;
    }
    1
}

/// Detour entry point for `FSocketBSD::SendTo(this, Data, Count, BytesSent, Dest)`.
///
/// Forwards untouched to original `FSocketBSD::SendTo`, then caches the Dedicated Server game
/// UDP socket handle and destination `SOCKADDR_STORAGE` when `socket == UIpNetDriver::GetSocket()`.
///
/// # Safety
/// Must only be invoked by UE4 through our verified `FSocketBSD::SendTo` detour.
pub unsafe extern "C" fn detour_socket_send_to(
    socket: *mut c_void,
    data: *const u8,
    count: i32,
    bytes_sent: *mut i32,
    dest: *const c_void,
) -> u8 {
    let orig_addr: usize = ORIG_SOCKET_SEND_TO.load(Ordering::Acquire);
    if orig_addr == 0 {
        return 0;
    }
    // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
    let orig_fn: SocketSendToFn =
        unsafe { std::mem::transmute::<usize, SocketSendToFn>(orig_addr) };
    // SAFETY: Always forward UE4's outgoing packet untouched first.
    let ok: u8 = unsafe { orig_fn(socket, data, count, bytes_sent, dest) };

    if ok != 0 && !socket.is_null() && !dest.is_null() && IN_DEDICATED_MATCH.load(Ordering::Acquire)
    {
        // SAFETY: `FSocketBSD::SocketType` (`u32`) is verified at `+0x08`.
        let socket_type: u32 = unsafe {
            *(socket
                .byte_add(OFFSET_FSOCKET_BSD_SOCKET_TYPE)
                .cast::<u32>())
        };
        if socket_type == FSOCKET_TYPE_DATAGRAM {
            // SAFETY: `FSocketBSD::Socket` (Win32 `SOCKET`) is verified at `+0x28`.
            let raw_socket: usize = unsafe {
                *(socket
                    .byte_add(OFFSET_FSOCKET_BSD_RAW_SOCKET)
                    .cast::<usize>())
            };
            // SAFETY: `dest` is a live `const FInternetAddrBSD*` from `FSocketBSD::SendTo`.
            unsafe {
                on_socket_send_to_observed(socket as usize, raw_socket, dest);
            }
        }
    }

    ok
}

/// Detour entry point for `FSocketBSD::Close(FSocketBSD* this)`.
///
/// Atomically invalidates any cached game UDP endpoint BEFORE invoking `closesocket`, preventing
/// the WASAPI capture thread from ever calling `sendto` on a closed or recycled socket handle.
///
/// # Safety
/// Must only be invoked by UE4 through our verified `FSocketBSD::Close` detour.
pub unsafe extern "C" fn detour_socket_close(socket: *mut c_void) -> u8 {
    if !socket.is_null() {
        on_socket_close_observed(socket as usize);
    }

    let orig_addr: usize = ORIG_SOCKET_CLOSE.load(Ordering::Acquire);
    if orig_addr == 0 {
        return 0;
    }
    // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
    let orig_fn: SocketCloseFn = unsafe { std::mem::transmute::<usize, SocketCloseFn>(orig_addr) };
    // SAFETY: Forwarding directly to original `FSocketBSD::Close`.
    unsafe { orig_fn(socket) }
}

/// Detour entry point for `FEngineLoop::Tick(FEngineLoop* this)`.
///
/// Drives the Plan A dual-mode state machine on the UE4 Game Thread:
/// - Main Menu / Lobby (`ADH_LobbyGameState` or no `ServerConnection`): keeps vanilla EOS Voice
///   fully active with zero WASAPI threads or microphone contention.
/// - Dedicated Server (`ADH_GameState` + `NetDriver->ServerConnection != null`): silences EOS
///   RTC, runs pure-Rust WASAPI + AEC3 + `RNNoise` + Opus TX over the existing game UDP port,
///   sends `DHV2::Hello` keepalives, and delivers decoded remote PCM into
///   `UDH_OnlineVoice::BeforeRecvAudioRendered` for native 3D spatial audio.
///
/// # Safety
/// Must only be invoked by UE4 through our verified `FEngineLoop::Tick` detour on the game thread.
pub unsafe extern "C" fn detour_engine_loop_tick(engine_loop: *mut c_void) {
    let orig_addr: usize = ORIG_ENGINE_LOOP_TICK.load(Ordering::Acquire);
    if orig_addr == 0 {
        return;
    }
    // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
    let orig_fn: EngineLoopTickFn =
        unsafe { std::mem::transmute::<usize, EngineLoopTickFn>(orig_addr) };
    // SAFETY: Execute UE4's full engine frame tick first so world/world-state pointers are live.
    unsafe {
        orig_fn(engine_loop);
    }

    let base: usize = MODULE_BASE.load(Ordering::Acquire);
    let image_size: usize = MODULE_IMAGE_SIZE.load(Ordering::Acquire);
    if base == 0 || image_size == 0 {
        return;
    }

    // SAFETY: Running on the UE4 Game Thread after `FEngineLoop::Tick`.
    let match_ctx_opt: Option<MatchContext> = unsafe { query_match_context(base) };
    let was_in_match: bool = IN_DEDICATED_MATCH.load(Ordering::Acquire);

    if let Some(ctx) = match_ctx_opt {
        if ctx.game_socket_ptr != 0 {
            set_match_game_socket_ptr(ctx.game_socket_ptr);
        }

        if was_in_match {
            let tick_num: u32 = MATCH_TICK_COUNTER
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_add(1);
            if tick_num.is_multiple_of(MATCH_REASSERT_TICK_INTERVAL) {
                // SAFETY: Periodic game-thread EOS re-mute and stale speaker pruning.
                unsafe {
                    maintain_dedicated_match_mode(base, &ctx);
                }
            }
        } else {
            IN_DEDICATED_MATCH.store(true, Ordering::Release);
            MATCH_TICK_COUNTER.store(0, Ordering::Release);
            // SAFETY: Entering Dedicated Server pure-Rust voice mode on the game thread.
            unsafe {
                enter_dedicated_match_mode(base, image_size, &ctx);
            }
        }

        // Keep local PTT gate synchronized defensively with `UDH_OnlineVoice::bWantsToTalk`.
        if !ctx.online_voice.is_null() {
            // SAFETY: `UDH_OnlineVoice::bWantsToTalk` is verified at `+0x80`.
            let wants: u8 = unsafe {
                *ctx.online_voice
                    .byte_add(OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK)
                    .cast::<u8>()
            };
            set_local_wants_to_talk(wants != 0);
        }

        let now: Instant = Instant::now();
        // SAFETY: Querying local player's `"SteamID|EOS_PUID"` string on game thread.
        let local_uid_opt: Option<String> = unsafe { query_local_player_uid_utf8(base, &ctx) };
        tick_hello_keepalive(local_uid_opt.as_deref(), now);

        // SAFETY: Draining paced `<= 100 ms` remote decoded frames and delivering into UE4 3D audio.
        unsafe {
            deliver_ready_remote_audio(base, &ctx, now);
        }

        // SAFETY: Broadcasting ESC-menu speaking icon ON/refresh/OFF state transitions on game thread.
        unsafe {
            dispatch_speaker_ui_notifications(base, &ctx, now);
        }
    } else if was_in_match {
        IN_DEDICATED_MATCH.store(false, Ordering::Release);
        // SAFETY: Exiting Dedicated Server back to Lobby/Main Menu on the game thread.
        unsafe {
            exit_dedicated_match_mode(base);
        }
    }
}

/// Detour entry point for `UDH_OnlineVoice::SetWantsToTalk(UDH_OnlineVoice* this, bool bWants)`.
///
/// # Safety
/// Must only be invoked by UE4 through our verified `UDH_OnlineVoice::SetWantsToTalk` detour on
/// the game thread with a live `UDH_OnlineVoice*` (or null).
pub unsafe extern "C" fn detour_set_wants_to_talk(online_voice: *mut c_void, wants_to_talk: u8) {
    let base: usize = MODULE_BASE.load(Ordering::Acquire);
    let in_dedicated: bool = IN_DEDICATED_MATCH.load(Ordering::Acquire)
        || (base != 0
            // SAFETY: Fallback check if PTT fires on the first frame before `FEngineLoop::Tick`.
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
    set_local_wants_to_talk(wants_flag != 0);
    if wants_flag == 0 {
        reset_local_mic_energy();
    }

    // SAFETY: Keep EOS RTC input muted while on a Dedicated Server.
    unsafe {
        silence_eos_voice_chat_user(base, online_voice);
    }
}

/// Detour entry point for `UDH_OnlineVoice::GetWantsToTalk(UDH_OnlineVoice* this) -> bool`.
///
/// # Safety
/// Must only be invoked by UE4 through our verified `UDH_OnlineVoice::GetWantsToTalk` detour on
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
/// Must only be invoked by UE4 through our verified `UDH_OnlineVoice::IsParticipantSpeaking`
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

    // Enforce manual/global mute and ghost isolation so muted players and isolated dead ghosts
    // never light up ESC-menu speaking icons or character lip-sync animations.
    // SAFETY: `ADH_PlayerState::bIsMuted` is verified at `+0x642`.
    let is_muted: u8 = unsafe { *player_state.byte_add(OFFSET_PS_IS_MUTED).cast::<u8>() };
    if is_muted != 0 {
        return 0;
    }
    if !local_ps.is_null() {
        // SAFETY: `ADH_PlayerState::bCanHearGhosts` (`+0x647`) and `bIsDeadGhost` (`+0x646`).
        let local_can_hear_ghosts: u8 =
            unsafe { *local_ps.byte_add(OFFSET_PS_CAN_HEAR_GHOSTS).cast::<u8>() };
        // SAFETY: Reading `ADH_PlayerState::bIsDeadGhost` (`+0x646`).
        let target_is_ghost: u8 =
            unsafe { *player_state.byte_add(OFFSET_PS_IS_GHOST).cast::<u8>() };
        if local_can_hear_ghosts == 0 && target_is_ghost != 0 {
            return 0;
        }
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
/// Must only be invoked by UE4 through our verified `UDH_OnlineVoice::IsPlayerInVoiceChat` detour
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

/// Detour entry point for `UDH_GameplayStatics::GetParticipantPositionalAudioEnergy(APlayerState*)`.
///
/// Drives Dread Hunger's HUD microphone volume meter (`UDH_VOIPTalkingWidget::NativeTick`, used by
/// both `UDH_PushToTalkWidget` and `UDH_LobbySlotSpeakingWidget`):
/// - Outside Dedicated Server matches: forwards to vanilla EOS RTC implementation.
/// - Dedicated Server matches: returns live smoothed post-DSP WASAPI energy (`0.0..=1.0`) for the
///   local player while `bWantsToTalk` is enabled, or smoothed decoded PCM energy (`0.0..=1.0`)
///   for unmuted, non-ghost-isolated remote speakers.
///
/// # Safety
/// Must only be invoked by UE4 through our verified
/// `UDH_GameplayStatics::GetParticipantPositionalAudioEnergy` detour on the game thread.
pub unsafe extern "C" fn detour_get_participant_audio_energy(player_state: *mut c_void) -> f32 {
    if !IN_DEDICATED_MATCH.load(Ordering::Acquire) {
        let orig_addr: usize = ORIG_GET_PARTICIPANT_AUDIO_ENERGY.load(Ordering::Acquire);
        if orig_addr == 0 {
            return 0.0;
        }
        // SAFETY: `orig_addr` points to our verified `RX` original-call trampoline.
        let orig_fn: GetParticipantAudioEnergyFn =
            unsafe { std::mem::transmute::<usize, GetParticipantAudioEnergyFn>(orig_addr) };
        // SAFETY: Forwarding Lobby/Menu mic meter query directly to vanilla EOS implementation.
        return unsafe { orig_fn(player_state) };
    }

    let base: usize = MODULE_BASE.load(Ordering::Acquire);
    if base == 0 || player_state.is_null() {
        return 0.0;
    }

    // SAFETY: Querying active Dedicated Server match pointers on the game thread.
    let Some(ctx): Option<MatchContext> = (unsafe { query_match_context(base) }) else {
        return 0.0;
    };

    // Local player microphone volume meter (lock-free atomic read from WASAPI post-DSP pipeline).
    if !ctx.local_ps.is_null() && player_state == ctx.local_ps {
        if !ctx.online_voice.is_null() {
            // SAFETY: `UDH_OnlineVoice::bWantsToTalk` is verified at `+0x80`.
            let wants: u8 = unsafe {
                *ctx.online_voice
                    .byte_add(OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK)
                    .cast::<u8>()
            };
            if wants == 0 {
                return 0.0;
            }
        }
        return read_local_mic_energy();
    }

    // Remote player meter: enforce manual/global mute and ghost isolation.
    // SAFETY: `ADH_PlayerState::bIsMuted` is verified at `+0x642`.
    let is_muted: u8 = unsafe { *player_state.byte_add(OFFSET_PS_IS_MUTED).cast::<u8>() };
    if is_muted != 0 {
        return 0.0;
    }
    if !ctx.local_ps.is_null() {
        // SAFETY: `ADH_PlayerState::bCanHearGhosts` (`+0x647`) and `bIsDeadGhost` (`+0x646`).
        let local_can_hear_ghosts: u8 = unsafe {
            *ctx.local_ps
                .byte_add(OFFSET_PS_CAN_HEAR_GHOSTS)
                .cast::<u8>()
        };
        // SAFETY: Reading `ADH_PlayerState::bIsDeadGhost` (`+0x646`).
        let target_is_ghost: u8 =
            unsafe { *player_state.byte_add(OFFSET_PS_IS_GHOST).cast::<u8>() };
        if local_can_hear_ghosts == 0 && target_is_ghost != 0 {
            return 0.0;
        }
    }

    // SAFETY: `APlayerState::UniqueId.UniqueNetId.Object` is at aligned `+0x258`.
    let unique_net_id: *const c_void = unsafe {
        *(player_state
            .byte_add(OFFSET_PS_UNIQUE_NET_ID_PTR)
            .cast::<*const c_void>())
    };
    if unique_net_id.is_null() {
        return 0.0;
    }

    let mut energy: f32 = 0.0;
    // SAFETY: Converting `FUniqueNetId` to `FString` and freeing UE4 memory via `FMemory::Free`.
    let _ok: bool = unsafe {
        with_unique_net_id_fstring(base, unique_net_id, |id_fstr: &FString| {
            if let Some(id_utf16) = fstring_to_utf16_vec(id_fstr) {
                energy = query_speaker_energy(&id_utf16);
            }
        })
    };
    energy
}

unsafe fn enter_dedicated_match_mode(base: usize, image_size: usize, ctx: &MatchContext) {
    if !ctx.online_voice.is_null() {
        // SAFETY: `UDH_OnlineVoice::bWantsToTalk` is verified at `+0x80`.
        let wants: u8 = unsafe {
            *ctx.online_voice
                .byte_add(OFFSET_DH_ONLINE_VOICE_WANTS_TO_TALK)
                .cast::<u8>()
        };
        set_local_wants_to_talk(wants != 0);
        // SAFETY: Muting EOS RTC devices on entering Dedicated Server match.
        unsafe {
            silence_eos_voice_chat_user(base, ctx.online_voice);
        }
    }

    clear_active_speakers();

    let opus_api_opt: Option<OpusApiTable> = if let Some(cached) = RESOLVED_OPUS_API.get() {
        Some(*cached)
    } else {
        // SAFETY: `base` and `image_size` were verified from live `DreadHunger-Win64-Shipping.exe`.
        match unsafe { OpusApiTable::resolve_from_client_image(base, image_size) } {
            Ok(table) => {
                let ver: String = table.version_string();
                let _stored: Result<(), OpusApiTable> = RESOLVED_OPUS_API.set(table);
                log_line(&format!(
                    "resolved in-process libopus successfully (version='{ver}')"
                ));
                Some(table)
            }
            Err(err) => {
                log_line(&format!(
                    "failed to resolve in-process libopus table: {err:?}"
                ));
                None
            }
        }
    };

    if let Some(opus_api) = opus_api_opt {
        start_match_session(opus_api);
    }

    log_line(
        "mode switch -> Dedicated Server (Plan A DHV2 pure-Rust AEC3+RNNoise+Opus active, EOS RTC silenced)",
    );
}

unsafe fn maintain_dedicated_match_mode(base: usize, ctx: &MatchContext) {
    if !ctx.online_voice.is_null() {
        // SAFETY: Periodic game-thread check ensuring EOS RTC remains muted during match.
        unsafe {
            reassert_eos_muted_if_needed(base, ctx.online_voice);
        }
    }
    prune_stale_speakers();
}

unsafe fn exit_dedicated_match_mode(base: usize) {
    stop_match_session();
    clear_active_speakers();

    // SAFETY: Restoring EOS RTC input/output unmuted for Lobby/Menu voice.
    if let Some(online_voice) = unsafe { query_any_online_voice(base) } {
        // SAFETY: `online_voice` is a live `UDH_OnlineVoice*` from the active `UDH_GameInstance`.
        unsafe {
            restore_eos_voice_chat_user(base, online_voice);
        }
    }
    log_line("mode switch -> Lobby/Menu (Plan A DHV2 stopped, mic released, EOS RTC restored)");
}

unsafe fn deliver_ready_remote_audio(base: usize, ctx: &MatchContext, now: Instant) {
    if ctx.online_voice.is_null() {
        return;
    }
    let mut frames: Vec<DecodedPlayoutFrame> = Vec::new();
    drain_ready_playout_frames(now, &mut frames);
    if frames.is_empty() {
        return;
    }

    let sample_len_i32: i32 = i32::try_from(OPUS_FRAME_SAMPLES_20MS).unwrap_or(960);
    let online_voice_slot: *mut c_void = ctx.online_voice;

    for mut frame in frames {
        if frame.uid_utf16_nul.len() < 2 {
            continue;
        }
        let Ok(num_i32): Result<i32, std::num::TryFromIntError> =
            i32::try_from(frame.uid_utf16_nul.len())
        else {
            continue;
        };

        let sender_fstr: FString = FString {
            data: frame.uid_utf16_nul.as_mut_ptr(),
            num: num_i32,
            max: num_i32,
        };

        // SAFETY: Resolving sender's `ADH_PlayerState*` on game thread using read-only `const FString&`.
        let target_ps: *mut c_void = unsafe {
            call_get_dh_ps_from_unique_id(base, &raw const sender_fstr, ctx.online_voice)
        };
        let no_nul_len: usize = frame.uid_utf16_nul.len() - 1;
        if !target_ps.is_null() {
            if target_ps == ctx.local_ps {
                continue;
            }
            // SAFETY: `ADH_PlayerState::bIsMuted` (manual + global mute) is verified at `+0x642`.
            let is_muted: u8 = unsafe { *target_ps.byte_add(OFFSET_PS_IS_MUTED).cast::<u8>() };
            if is_muted != 0 {
                mark_speaker_inactive(&frame.uid_utf16_nul[..no_nul_len], now);
                continue;
            }
            if !ctx.local_ps.is_null() {
                // SAFETY: Ghost isolation fields verified at `+0x646` / `+0x647`.
                let local_can_hear_ghosts: u8 = unsafe {
                    *ctx.local_ps
                        .byte_add(OFFSET_PS_CAN_HEAR_GHOSTS)
                        .cast::<u8>()
                };
                // SAFETY: Reading `ADH_PlayerState::bIsDeadGhost` (`+0x646`).
                let target_is_ghost: u8 =
                    unsafe { *target_ps.byte_add(OFFSET_PS_IS_GHOST).cast::<u8>() };
                if local_can_hear_ghosts == 0 && target_is_ghost != 0 {
                    mark_speaker_inactive(&frame.uid_utf16_nul[..no_nul_len], now);
                    continue;
                }
            }
        }

        let frame_energy: f32 = pcm_i16_rms_energy(&frame.pcm_i16);
        if has_audible_speech(&frame.pcm_i16) {
            record_active_speaker(
                frame.uid_utf16_nul[..no_nul_len].to_vec(),
                frame_energy,
                now,
            );
        } else {
            decay_existing_speaker_energy(&frame.uid_utf16_nul[..no_nul_len]);
        }

        let pcm_view: TArrayViewI16 = TArrayViewI16 {
            data: frame.pcm_i16.as_ptr(),
            num: sample_len_i32,
            pad: 0,
        };
        // SAFETY: Injecting decoded `48 kHz` mono `i16` PCM directly into Dread Hunger's 3D/2D
        // spatial audio pipeline (`UDH_OnlineVoice::BeforeRecvAudioRendered`).
        unsafe {
            call_before_recv_audio_rendered(
                base,
                &raw const online_voice_slot,
                &raw const sender_fstr,
                &raw const pcm_view,
            );
        }
    }
}

unsafe fn query_local_player_uid_utf8(base: usize, ctx: &MatchContext) -> Option<String> {
    if ctx.local_ps.is_null() {
        return None;
    }
    // SAFETY: `APlayerState::UniqueId.UniqueNetId.Object` is at aligned `+0x258`.
    let unique_net_id: *const c_void = unsafe {
        *(ctx
            .local_ps
            .byte_add(OFFSET_PS_UNIQUE_NET_ID_PTR)
            .cast::<*const c_void>())
    };
    if unique_net_id.is_null() {
        return None;
    }

    let mut result: Option<String> = None;
    // SAFETY: Converting local `FUniqueNetId` to `FString` and freeing UE4 memory via `FMemory::Free`.
    let _ok: bool = unsafe {
        with_unique_net_id_fstring(base, unique_net_id, |id_fstr: &FString| {
            if let Some(id_utf16) = fstring_to_utf16_vec(id_fstr) {
                let s: String = String::from_utf16_lossy(&id_utf16);
                if !s.is_empty() && !s.contains('\0') && s.len() <= dh_voice_proto::MAX_UID_LEN {
                    result = Some(s);
                }
            }
        })
    };
    result
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

    // SAFETY: `UDH_GameInstance::MessageBus` is verified at aligned `+0x518`.
    let message_bus: *mut c_void = unsafe {
        *(game_instance
            .byte_add(OFFSET_DH_GAME_INSTANCE_MESSAGE_BUS)
            .cast::<*mut c_void>())
    };

    // SAFETY: `UDH_GameInstance::OnlineVoice` is verified at aligned `+0x520`.
    let online_voice: *mut c_void = unsafe {
        *(game_instance
            .byte_add(OFFSET_DH_GAME_INSTANCE_ONLINE_VOICE)
            .cast::<*mut c_void>())
    };

    // Resolve the exact `FSocketBSD*` owned by `UIpNetDriver::GetSocket()` (`0x909B80`).
    // SAFETY: Verifying `NetDriver->IsA(UIpNetDriver::StaticClass())` before invoking `GetSocket`.
    let game_socket_ptr: usize =
        if unsafe { uobject_is_a(base, net_driver, RVA_UIPNETDRIVER_STATIC_CLASS) } {
            // SAFETY: `net_driver` is a verified `UIpNetDriver*` on the game thread.
            unsafe { call_ip_net_driver_get_socket(base, net_driver) as usize }
        } else {
            0
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
        message_bus,
        online_voice,
        local_pc: local_controller,
        local_ps: local_player_state,
        game_socket_ptr,
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
    if char_len == 0 || char_len > MAX_UE4_UID_UTF16_LEN {
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

/// Evaluates tracked remote speakers against `now` and returns NUL-terminated UTF-16LE UIDs
/// requiring a `UDH_MessageBus::OnVoiceChatParticipantChangedDelegate` broadcast:
/// - Instant ON when an active speaker (`elapsed < SPEAKER_ACTIVE_WINDOW_MS`) has not yet been
///   notified.
/// - Periodic refresh every `SPEAKER_UI_REFRESH_INTERVAL_MS` (`250 ms`) while active so VOIP
///   range entry/exit, Thrall Hush, and Mute updates propagate promptly.
/// - Instant OFF right at `elapsed >= SPEAKER_ACTIVE_WINDOW_MS` (`350 ms`) with zero repeated
///   OFF spam afterwards.
#[must_use]
pub fn collect_speaker_ui_notifications(
    speakers: &mut [SpeakerEntry],
    now: Instant,
) -> Vec<Vec<u16>> {
    let active_window: Duration = Duration::from_millis(SPEAKER_ACTIVE_WINDOW_MS);
    let refresh_interval: Duration = Duration::from_millis(SPEAKER_UI_REFRESH_INTERVAL_MS);
    let mut notifications: Vec<Vec<u16>> = Vec::new();

    for entry in speakers.iter_mut() {
        if entry.id_utf16.is_empty() {
            continue;
        }
        let is_active: bool = now.saturating_duration_since(entry.last_heard) < active_window;
        if is_active {
            let should_broadcast: bool = if entry.notified_speaking {
                now.saturating_duration_since(entry.last_notify_at) >= refresh_interval
            } else {
                entry.notified_speaking = true;
                true
            };
            if should_broadcast {
                entry.last_notify_at = now;
                let mut uid_nul: Vec<u16> = Vec::with_capacity(entry.id_utf16.len() + 1);
                uid_nul.extend_from_slice(&entry.id_utf16);
                uid_nul.push(0u16);
                notifications.push(uid_nul);
            }
        } else if entry.notified_speaking {
            entry.notified_speaking = false;
            entry.last_notify_at = now;
            entry.energy = 0.0;
            let mut uid_nul: Vec<u16> = Vec::with_capacity(entry.id_utf16.len() + 1);
            uid_nul.extend_from_slice(&entry.id_utf16);
            uid_nul.push(0u16);
            notifications.push(uid_nul);
        }
    }

    notifications
}

unsafe fn dispatch_speaker_ui_notifications(base: usize, ctx: &MatchContext, now: Instant) {
    if ctx.message_bus.is_null() {
        return;
    }

    // Release `ACTIVE_SPEAKERS` lock BEFORE invoking UE4 delegates to eliminate any re-entrancy
    // deadlock when `UDH_LobbySlotSpeakingWidget::Update` calls back into `IsParticipantSpeaking`.
    let pending: Vec<Vec<u16>> = {
        let Ok(mut speakers): Result<std::sync::MutexGuard<'_, Vec<SpeakerEntry>>, _> =
            ACTIVE_SPEAKERS.lock()
        else {
            return;
        };
        collect_speaker_ui_notifications(&mut speakers, now)
    };

    if pending.is_empty() {
        return;
    }

    for mut uid_utf16_nul in pending {
        if uid_utf16_nul.len() < 2 {
            continue;
        }
        let Ok(num_i32): Result<i32, std::num::TryFromIntError> =
            i32::try_from(uid_utf16_nul.len())
        else {
            continue;
        };
        let mut participant_fstr: FString = FString {
            data: uid_utf16_nul.as_mut_ptr(),
            num: num_i32,
            max: num_i32,
        };
        // SAFETY: `ctx.message_bus` is a live `UDH_MessageBus*` on the game thread and
        // `participant_fstr` is a valid read-only stack `FString` deep-copied by UE4's delegate exec.
        unsafe {
            call_broadcast_voice_chat_participant_changed(
                base,
                ctx.message_bus,
                &raw mut participant_fstr,
            );
        }
    }
}

fn record_active_speaker(id_utf16: Vec<u16>, frame_energy: f32, now: Instant) {
    let Ok(mut speakers): Result<std::sync::MutexGuard<'_, Vec<SpeakerEntry>>, _> =
        ACTIVE_SPEAKERS.lock()
    else {
        return;
    };
    for entry in speakers.iter_mut() {
        if entry.id_utf16 == id_utf16 {
            entry.last_heard = now;
            entry.energy = smooth_meter_energy(
                entry.energy,
                frame_energy,
                REMOTE_ENERGY_ATTACK_ALPHA,
                REMOTE_ENERGY_RELEASE_ALPHA,
            );
            return;
        }
    }
    if speakers.len() >= MAX_TRACKED_SPEAKERS {
        let cutoff: Duration = Duration::from_millis(SPEAKER_PRUNE_WINDOW_MS);
        speakers.retain(|entry: &SpeakerEntry| {
            entry.notified_speaking || now.duration_since(entry.last_heard) <= cutoff
        });
    }
    if speakers.len() < MAX_TRACKED_SPEAKERS {
        let initial_energy: f32 = smooth_meter_energy(
            0.0,
            frame_energy,
            REMOTE_ENERGY_ATTACK_ALPHA,
            REMOTE_ENERGY_RELEASE_ALPHA,
        );
        speakers.push(SpeakerEntry {
            id_utf16,
            last_heard: now,
            energy: initial_energy,
            notified_speaking: false,
            last_notify_at: now,
        });
    }
}

fn mark_speaker_inactive(id_utf16: &[u16], now: Instant) {
    let Ok(mut speakers): Result<std::sync::MutexGuard<'_, Vec<SpeakerEntry>>, _> =
        ACTIVE_SPEAKERS.lock()
    else {
        return;
    };
    for entry in speakers.iter_mut() {
        if entry.id_utf16.as_slice() == id_utf16 {
            entry.energy = 0.0;
            if let Some(expired) = now.checked_sub(Duration::from_millis(SPEAKER_ACTIVE_WINDOW_MS))
            {
                entry.last_heard = expired;
            }
            return;
        }
    }
}

fn decay_existing_speaker_energy(id_utf16: &[u16]) {
    let Ok(mut speakers): Result<std::sync::MutexGuard<'_, Vec<SpeakerEntry>>, _> =
        ACTIVE_SPEAKERS.lock()
    else {
        return;
    };
    for entry in speakers.iter_mut() {
        if entry.id_utf16.as_slice() == id_utf16 {
            entry.energy = smooth_meter_energy(
                entry.energy,
                0.0,
                REMOTE_ENERGY_ATTACK_ALPHA,
                REMOTE_ENERGY_RELEASE_ALPHA,
            );
            return;
        }
    }
}

fn query_speaker_energy(id_utf16: &[u16]) -> f32 {
    let Ok(speakers): Result<std::sync::MutexGuard<'_, Vec<SpeakerEntry>>, _> =
        ACTIVE_SPEAKERS.lock()
    else {
        return 0.0;
    };
    let now: Instant = Instant::now();
    for entry in speakers.iter() {
        if entry.id_utf16.as_slice() == id_utf16 {
            let elapsed_ms: u64 =
                u64::try_from(now.duration_since(entry.last_heard).as_millis()).unwrap_or(u64::MAX);
            return decay_meter_energy(entry.energy, elapsed_ms, SPEAKER_ACTIVE_WINDOW_MS);
        }
    }
    0.0
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
            return now.duration_since(entry.last_heard) < window;
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
    speakers.retain(|entry: &SpeakerEntry| {
        entry.notified_speaking || now.duration_since(entry.last_heard) <= cutoff
    });
}

fn clear_active_speakers() {
    if let Ok(mut speakers) = ACTIVE_SPEAKERS.lock() {
        speakers.clear();
    }
}

unsafe fn call_ip_net_driver_get_socket(base: usize, net_driver: *mut c_void) -> *mut c_void {
    let Some(addr): Option<usize> = base.checked_add(RVA_UIPNETDRIVER_GET_SOCKET) else {
        return std::ptr::null_mut();
    };
    // SAFETY: Verified `UIpNetDriver::GetSocket` address & signature.
    let func: GetNetDriverSocketFn =
        unsafe { std::mem::transmute::<usize, GetNetDriverSocketFn>(addr) };
    // SAFETY: `net_driver` was verified via `IsA(UIpNetDriver::StaticClass())`.
    unsafe { func(net_driver) }
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

unsafe fn call_broadcast_voice_chat_participant_changed(
    base: usize,
    message_bus: *mut c_void,
    participant_id_str: *mut FString,
) {
    if message_bus.is_null() || participant_id_str.is_null() {
        return;
    }
    // SAFETY: `UDH_MessageBus::OnVoiceChatParticipantChangedDelegate` is verified at `+0x398`.
    let delegate_ptr: *mut c_void =
        unsafe { message_bus.byte_add(OFFSET_DH_MESSAGE_BUS_ON_VOICE_CHAT_PARTICIPANT_CHANGED) };
    // SAFETY: `InvocationList.ArrayNum` (`i32`) is verified at delegate offset `+0x08`.
    let listener_count: i32 = unsafe {
        *(delegate_ptr
            .byte_add(OFFSET_MULTICAST_DELEGATE_NUM)
            .cast::<i32>())
    };
    if listener_count <= 0 || listener_count > MAX_SANE_DELEGATE_LISTENERS {
        return;
    }
    let Some(addr): Option<usize> = base.checked_add(RVA_PROCESS_MULTICAST_DELEGATE) else {
        return;
    };
    // SAFETY: Verified `TMulticastScriptDelegate<FWeakObjectPtr>::ProcessMulticastDelegate<UObject>`
    // address (`0x7619D0`) and 2-pointer Win64 ABI signature matching `0x140EA001B`.
    let func: ProcessMulticastDelegateFn =
        unsafe { std::mem::transmute::<usize, ProcessMulticastDelegateFn>(addr) };
    // SAFETY: Firing multicast delegate on the UE4 Game Thread with live stack `FString` parameter.
    unsafe {
        func(delegate_ptr, participant_id_str.cast::<c_void>());
    }
}
