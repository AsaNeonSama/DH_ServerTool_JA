use crate::consts::patch::{SET_LOBBY_ATTR_SITE_LEN, X64_CALL_MEM_REL32_LEN, X64_CALL_REL32_LEN};

/// Single verified in-place byte patch in the client executable image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytePatch {
    /// Human-readable identifier of the patch site.
    pub name: &'static str,
    /// Relative virtual address from `DreadHunger-Win64-Shipping.exe` base.
    pub rva: usize,
    /// Exact original machine-code bytes required before patching.
    pub expected: &'static [u8],
    /// Replacement machine-code bytes (must match `expected.len()`).
    pub replacement: &'static [u8],
}

/// Specification for detouring `FOnlineSessionSettings::Get<FString>` inside
/// `UDH_OnlineSession::PostUpdate` so `UDH_GameInstance::ResetURI` runs whenever the EOS lobby's
/// `SessionSettingURIName` attribute is absent or empty (`ArrayNum <= 1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostUpdateUriResetSpec {
    /// Human-readable identifier of the trampoline patch.
    pub name: &'static str,
    /// RVA of the original 5-byte `call rel32` instruction.
    pub call_site_rva: usize,
    /// Exact expected 5 bytes of the original `call rel32` instruction.
    pub expected_call_bytes: [u8; X64_CALL_REL32_LEN],
    /// RVA of `FOnlineSessionSettings::Get<FString>`.
    pub original_get_rva: usize,
    /// RVA of `UDH_GameInstance::GetInstance(const UObject*)`.
    pub get_game_instance_rva: usize,
    /// RVA of `UDH_GameInstance::ResetURI()`.
    pub reset_uri_rva: usize,
}

/// Specification for detouring `UDH_OnlineSession::IsSessionOwner()` inside match-end / lobby-reset
/// routines (`OnMatchEnded`, `ResetCurrentData`) so the host clears `UDH_GameInstance + 0x570`
/// immediately before pushing the cleared URI to EOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerResetTrampolineSpec {
    /// Human-readable identifier of the trampoline patch.
    pub name: &'static str,
    /// RVA of the original 5-byte `call rel32` instruction.
    pub call_site_rva: usize,
    /// Exact expected 5 bytes of the original `call rel32` instruction.
    pub expected_call_bytes: [u8; X64_CALL_REL32_LEN],
    /// RVA of `UDH_OnlineSession::IsSessionOwner()`.
    pub is_session_owner_rva: usize,
    /// RVA of `UDH_GameInstance::GetInstance(const UObject*)`.
    pub get_game_instance_rva: usize,
    /// RVA of `UDH_GameInstance::ResetURI()`.
    pub reset_uri_rva: usize,
}

/// Specification for detouring `UDH_OnlineSession::Update()` inside `UDH_OnlineSession::SetURI()`
/// (`0xE7E386`) so `ConnectToIP` sets `UDH_OnlineSession::StartUTCDateTime` (`+0x48`) to the
/// current Unix UTC timestamp (`>= 1`) before pushing `URI` and `STARTUTCDATETIME` to EOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetUriStartTimeTrampolineSpec {
    /// Human-readable identifier of the trampoline patch.
    pub name: &'static str,
    /// RVA of the original 5-byte `call rel32` instruction (`call UDH_OnlineSession::Update`).
    pub call_site_rva: usize,
    /// Exact expected 5 bytes of the original `call rel32` instruction.
    pub expected_call_bytes: [u8; X64_CALL_REL32_LEN],
    /// RVA of `FDateTime::UtcNow(FDateTime* out)`.
    pub utc_now_rva: usize,
    /// RVA of `UDH_OnlineSession::Update()`.
    pub update_session_rva: usize,
}

/// Specification for detouring the indirect IAT call to `EOS_LobbyModification_AddAttribute`
/// inside `FOnlineSessionEOS::AddLobbyAttribute` (`0x9CA894`) so Plan B (`feature = "private-uri"`)
/// marks only the `"URI"` attribute as `EOS_LAT_PRIVATE (1)` (member-only) while keeping all
/// other attributes `EOS_LAT_PUBLIC (0)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddLobbyAttributePrivateUriSpec {
    /// Human-readable identifier of the trampoline patch.
    pub name: &'static str,
    /// RVA of the original 6-byte `call qword ptr [rip + rel32]` instruction.
    pub call_site_rva: usize,
    /// Exact expected 6 bytes of the original indirect `call` instruction.
    pub expected_call_bytes: [u8; X64_CALL_MEM_REL32_LEN],
    /// RVA of the `EOS_LobbyModification_AddAttribute` IAT pointer slot.
    pub iat_slot_rva: usize,
}

/// Specification for replacing the 26-byte `strncpy(..., 0x101)` site inside
/// `FOnlineSessionEOS::SetLobbyAttributes` (`0xA03820..0xA0383A`) with a trampoline that copies
/// up to `1000` UTF-8 bytes (without splitting multi-byte UTF-8 codepoints) into a dedicated
/// `PAGE_READWRITE` buffer and updates `EOS_Lobby_AttributeData::Value.AsUtf8` (`[rbp+0x80]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetLobbyAttributesMaxLenSpec {
    /// Human-readable identifier of the trampoline patch.
    pub name: &'static str,
    /// RVA of the original 26-byte `strncpy` call sequence (`0xA03820`).
    pub site_rva: usize,
    /// Exact expected 26 bytes of the original `strncpy` sequence.
    pub expected_site_bytes: [u8; SET_LOBBY_ATTR_SITE_LEN],
}

/// Specification for detouring the `0xE7EDC9` helper call inside
/// `UDH_OnlineSession::OnDestroySessionComplete` so `UDH_OnlineSession::bHasGameStarted`
/// (`[this + 0x16E]`) is cleared to `0` before tail-jumping to the original callee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnDestroyClearStartedSpec {
    /// Human-readable identifier of the trampoline patch.
    pub name: &'static str,
    /// RVA of the original 5-byte `call rel32` instruction.
    pub call_site_rva: usize,
    /// Exact expected 5 bytes of the original `call rel32` instruction.
    pub expected_call_bytes: [u8; X64_CALL_REL32_LEN],
    /// RVA of the original callee (`0xF072B0`).
    pub original_callee_rva: usize,
}

/// Specification for detouring `UDH_OnlineSession::Update()` inside
/// `UDH_OnlineSession::OnJoinSessionComplete` (`0xE815D7`) so reconnecting players clear stale
/// `UDH_GameInstance + 0x570` (`ResetURI`) once upon successful join before fetching fresh lobby
/// attributes and triggering travel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnJoinResetUriSpec {
    /// Human-readable identifier of the trampoline patch.
    pub name: &'static str,
    /// RVA of the original 5-byte `call rel32` instruction (`call UDH_OnlineSession::Update`).
    pub call_site_rva: usize,
    /// Exact expected 5 bytes of the original `call rel32` instruction.
    pub expected_call_bytes: [u8; X64_CALL_REL32_LEN],
    /// RVA of `UDH_GameInstance::GetInstance(const UObject*)`.
    pub get_game_instance_rva: usize,
    /// RVA of `UDH_GameInstance::ResetURI()`.
    pub reset_uri_rva: usize,
    /// RVA of `UDH_OnlineSession::Update()`.
    pub update_session_rva: usize,
}

/// Specification for detouring `IsLobbyMap` (`0xF0AF50`) inside
/// `UDH_OnlineSession::OnSessionParticipantsChange` (`0xE85B28`) so leaving players are NOT
/// erased from the lobby `MEMBERS` list once `UDH_OnlineSession::StartUTCDateTime` (`+0x48`) is
/// non-zero (match already started).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnParticipantsKeepMembersSpec {
    /// Human-readable identifier of the trampoline patch.
    pub name: &'static str,
    /// RVA of the original 5-byte `call rel32` instruction (`call IsLobbyMap`).
    pub call_site_rva: usize,
    /// Exact expected 5 bytes of the original `call rel32` instruction.
    pub expected_call_bytes: [u8; X64_CALL_REL32_LEN],
    /// RVA of `IsLobbyMap` (`0xF0AF50`).
    pub is_lobby_map_rva: usize,
}

/// Outcome of attempting to verify and apply a single patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOutcome {
    /// Original bytes matched and replacement was written.
    Applied,
    /// Target memory already contained the replacement bytes.
    AlreadyApplied,
    /// RVA or byte length fell outside the mapped image bounds.
    OutOfBounds,
    /// Original bytes did not match expected signature; safely skipped.
    SignatureMismatch,
    /// Page protection or nearby trampoline allocation failed.
    MemoryError,
}

/// Aggregate result of applying all client patches at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchReport {
    /// Whether the host process matched `DreadHunger-Win64-Shipping.exe`.
    pub target_matched: bool,
    /// Number of patches newly applied.
    pub applied: usize,
    /// Number of patches already present.
    pub already_applied: usize,
    /// Number of patches skipped due to mismatch, bounds, or memory failure.
    pub skipped: usize,
}

/// Error returned while encoding a `call rel32` instruction or nearby trampoline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrampolineError {
    /// Computed relative displacement does not fit in a signed 32-bit offset.
    DisplacementOutOfRange,
    /// Address arithmetic overflowed `usize`.
    AddressOverflow,
}
