use std::ffi::c_void;
use std::time::Instant;

/// Snapshot of live UE4 pointers while connected to a Dread Hunger Dedicated Server (`ADH_GameState`).
#[derive(Debug, Clone, Copy)]
pub struct MatchContext {
    /// Live game `UWorld*`.
    pub world: *mut c_void,
    /// Live `UDH_GameInstance*`.
    pub game_instance: *mut c_void,
    /// Live `UDH_MessageBus*` (`game_instance + 0x518`).
    pub message_bus: *mut c_void,
    /// Live `UDH_OnlineVoice*` (`game_instance + 0x520`).
    pub online_voice: *mut c_void,
    /// Live local `APlayerController*` (may be null briefly during seamless transitions).
    pub local_pc: *mut c_void,
    /// Live local `ADH_PlayerState*` (may be null briefly before `PlayerState` replication).
    pub local_ps: *mut c_void,
    /// Live `FSocketBSD*` pointer returned by `UIpNetDriver::GetSocket()` (`0` if unavailable).
    pub game_socket_ptr: usize,
}

/// Timestamped remote speaking activity record keyed by UTF-16 `UniqueNetId` string.
#[derive(Debug, Clone)]
pub struct SpeakerEntry {
    /// Exact UTF-16LE player unique ID (`"SteamID|EOS_PUID"` without trailing NUL).
    pub id_utf16: Vec<u16>,
    /// Monotonic timestamp of the most recent audible decoded voice frame.
    pub last_heard: Instant,
    /// Smoothed perceptual audio energy (`0.0..=1.0`) feeding HUD volume meters.
    pub energy: f32,
    /// Whether `UDH_MessageBus::OnVoiceChatParticipantChangedDelegate` currently considers this
    /// remote speaker active (controls ESC-menu speaking icon ON/refresh/OFF broadcasts).
    pub notified_speaking: bool,
    /// Monotonic timestamp of the most recent delegate broadcast emitted for this speaker.
    pub last_notify_at: Instant,
}
