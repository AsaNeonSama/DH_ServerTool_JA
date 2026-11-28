use crate::structs::patch::{DetourSpec, EosRecvGuardSpec};

/// Target executable basename guarded before applying any in-memory patch.
pub const TARGET_PROCESS_NAME: &str = "DreadHunger-Win64-Shipping.exe";

/// Environment variable that enables opt-in runtime diagnostic logging (`DH_VOICE_LOG=1`).
pub const VOICE_LOG_ENV_VAR: &str = "DH_VOICE_LOG";

/// Relative diagnostic log path written only when `DH_VOICE_LOG=1` is set.
pub const VOICE_LOG_FILE_NAME: &str = "dh_voice_client.log";

/// PE DOS header magic (`"MZ"`).
pub const PE_DOS_MAGIC: u16 = 0x5A4D;
/// Offset of `e_lfanew` inside `IMAGE_DOS_HEADER`.
pub const PE_LFANEW_OFFSET: usize = 0x3C;
/// Minimum DOS header byte length required to read `e_lfanew`.
pub const PE_MIN_DOS_BYTES: usize = 0x40;
/// PE NT header signature (`"PE\0\0"`).
pub const PE_NT_SIGNATURE: u32 = 0x0000_4550;
/// Offset of `OptionalHeader` relative to `IMAGE_NT_HEADERS64`.
pub const PE_OPTIONAL_HEADER_OFFSET: usize = 0x18;
/// Optional header magic for PE32+ (64-bit).
pub const PE_OPT_MAGIC_PE32_PLUS: u16 = 0x020B;
/// Offset of `SizeOfImage` relative to `IMAGE_OPTIONAL_HEADER64`.
pub const PE_SIZE_OF_IMAGE_OFFSET: usize = 0x38;
/// Minimum NT header byte length required to read `SizeOfImage`.
pub const PE_MIN_NT_BYTES: usize = PE_OPTIONAL_HEADER_OFFSET + PE_SIZE_OF_IMAGE_OFFSET + 4;

/// Size in bytes of the single nearby `RX` trampoline page holding all client voice stubs.
pub const TRAMPOLINE_ALLOC_SIZE: usize = 0x1000;
/// Alignment stride between adjacent stubs inside the shared `RX` trampoline page.
pub const TRAMPOLINE_SLOT_ALIGN: usize = 0x10;
/// Fallback Win32 allocation granularity when `GetSystemInfo` reports zero (`64 KiB`).
pub const DEFAULT_ALLOC_GRANULARITY: usize = 0x10000;
/// Maximum search distance (`~1.75 GiB`) from the executable image for 32-bit relative jumps.
pub const TRAMPOLINE_MAX_DISTANCE: usize = 0x7000_0000;

/// Opcode for x86-64 `jmp rel32`.
pub const X64_JMP_REL32_OPCODE: u8 = 0xE9;
/// Byte length of x86-64 `jmp rel32`.
pub const X64_JMP_REL32_LEN: usize = 5;
/// Opcode bytes for x86-64 `jmp qword ptr [rip + 0]`.
pub const X64_JMP_ABS_RIP0_PREFIX: [u8; 6] = [0xFF, 0x25, 0x00, 0x00, 0x00, 0x00];
/// Byte length of x86-64 `jmp qword ptr [rip + 0]; dq target_addr`.
pub const X64_ABS_JMP_LEN: usize = 14;
/// Opcode prefix for `mov rax, imm64`.
pub const X64_MOV_RAX_IMM64_PREFIX: [u8; 2] = [0x48, 0xB8];
/// x86-64 `int3` padding byte used to fill unused executable trampoline page space.
pub const X64_INT3_BYTE: u8 = 0xCC;
/// x86-64 `nop` byte used to pad overwritten instructions beyond 5 bytes.
pub const X64_NOP_BYTE: u8 = 0x90;

/// Machine code suffix after `mov rax, &IN_DEDICATED_MATCH` in the `EOSPlus` recv guard stub:
/// ```text
/// cmp byte ptr [rax], 0         ; 80 38 00
/// je  +1                        ; 74 01 (skip `ret` and execute stolen prologue + resume)
/// ret                           ; C3    (drop EOS RTC audio silently while on Dedicated Server)
/// ```
pub const EOS_RECV_GUARD_CHECK: [u8; 6] = [0x80, 0x38, 0x00, 0x74, 0x01, 0xC3];

/// Exact byte length of the `FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered` guard
/// stub (`10 + 6 + 10 + 14 = 40`).
pub const EOS_RECV_GUARD_STUB_LEN: usize = 40;

/// Number of standard function-entry detours installed by the Plan A voice engine (`8`).
pub const STANDARD_DETOUR_COUNT: usize = 8;

/// Total number of client detours applied at startup (`1` guard detour + `8` function detours = `9`).
pub const TOTAL_PATCH_COUNT: usize = 1 + STANDARD_DETOUR_COUNT;

/// Guard detour on `FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered` (`0xA2DC70`)
/// suppressing EOS RTC playback while on a Dedicated Server (prevents any echo/double audio).
pub const EOS_BEFORE_RECV_AUDIO_GUARD: EosRecvGuardSpec = EosRecvGuardSpec {
    name: "FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered match guard",
    rva: 0x00A2_DC70,
    expected_prologue: [0x48, 0x8B, 0xC4, 0x48, 0x81, 0xEC, 0x88, 0x00, 0x00, 0x00],
};

/// Detour on `FSocketBSD::RecvFrom` (`0x1666300`) demultiplexing `DHV2` voice datagrams
/// on the existing game UDP port before UE4 `UNetConnection` sees them.
pub const DETOUR_SOCKET_RECV_FROM: DetourSpec = DetourSpec {
    name: "FSocketBSD::RecvFrom DHV2 demux",
    rva: 0x0166_6300,
    expected_prologue: &[0x48, 0x89, 0x5C, 0x24, 0x08],
};

/// Detour on `FSocketBSD::SendTo` (`0x1666530`) capturing the active Dedicated Server UDP
/// socket handle and destination `SOCKADDR_STORAGE`.
pub const DETOUR_SOCKET_SEND_TO: DetourSpec = DetourSpec {
    name: "FSocketBSD::SendTo endpoint capture",
    rva: 0x0166_6530,
    expected_prologue: &[0x48, 0x89, 0x5C, 0x24, 0x08],
};

/// Detour on `FSocketBSD::Close` (`0x1660B10`, exact 6-byte prologue before `+0x11` short `je`)
/// clearing the cached UDP endpoint before Winsock closes/recycles the handle.
pub const DETOUR_SOCKET_CLOSE: DetourSpec = DetourSpec {
    name: "FSocketBSD::Close endpoint cleanup",
    rva: 0x0166_0B10,
    expected_prologue: &[0x40, 0x53, 0x48, 0x83, 0xEC, 0x20],
};

/// Detour on `FEngineLoop::Tick` (`0x74E7E0`) driving the dual-mode state machine, `DHV2::Hello`
/// keepalives, and paced remote PCM delivery into `UDH_OnlineVoice::BeforeRecvAudioRendered`.
pub const DETOUR_ENGINE_LOOP_TICK: DetourSpec = DetourSpec {
    name: "FEngineLoop::Tick voice engine driver",
    rva: 0x0074_E7E0,
    expected_prologue: &[0x48, 0x89, 0x4C, 0x24, 0x08],
};

/// Detour on `UDH_OnlineVoice::SetWantsToTalk` (`0xEC8FE0`).
pub const DETOUR_SET_WANTS_TO_TALK: DetourSpec = DetourSpec {
    name: "UDH_OnlineVoice::SetWantsToTalk PTT bridge",
    rva: 0x00EC_8FE0,
    expected_prologue: &[0x40, 0x53, 0x57, 0x41, 0x56],
};

/// Detour on `UDH_OnlineVoice::GetWantsToTalk` (`0xEB1F60`).
pub const DETOUR_GET_WANTS_TO_TALK: DetourSpec = DetourSpec {
    name: "UDH_OnlineVoice::GetWantsToTalk PTT query",
    rva: 0x00EB_1F60,
    expected_prologue: &[0x48, 0x89, 0x5C, 0x24, 0x18],
};

/// Detour on `UDH_OnlineVoice::IsParticipantSpeaking` (`0xEB4F90`).
pub const DETOUR_IS_PARTICIPANT_SPEAKING: DetourSpec = DetourSpec {
    name: "UDH_OnlineVoice::IsParticipantSpeaking HUD indicator",
    rva: 0x00EB_4F90,
    expected_prologue: &[0x48, 0x89, 0x5C, 0x24, 0x08],
};

/// Detour on `UDH_OnlineVoice::IsPlayerInVoiceChat` (`0xEB5140`).
pub const DETOUR_IS_PLAYER_IN_VOICE_CHAT: DetourSpec = DetourSpec {
    name: "UDH_OnlineVoice::IsPlayerInVoiceChat Tab-menu mute enabler",
    rva: 0x00EB_5140,
    expected_prologue: &[0x48, 0x89, 0x5C, 0x24, 0x10],
};
