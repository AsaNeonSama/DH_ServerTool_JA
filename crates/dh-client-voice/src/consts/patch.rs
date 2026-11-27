use crate::structs::patch::{
    BytePatch, DetourSpec, EosRecvGuardSpec, SubmitRemoteVoiceSpec, VtablePatchSpec,
};

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

/// Prologue machine code for the `SubmitRemoteVoiceData` (`0x927C0C`) naked relay stub:
/// ```text
/// sub rsp, 0x20                 ; 48 83 EC 20 (rsp is already 16-byte aligned at 0x927C0C)
/// mov rcx, r13                  ; 4C 89 E9    (arg1 = *const FUniqueNetIdWrapper)
/// mov rdx, qword ptr [r14+0x140]; 49 8B 96 40 01 00 00 (arg2 = *const u8 decoded PCM)
/// mov r8d, dword ptr [rbp+0x80] ; 44 8B 85 80 00 00 00 (arg3 = u32 decoded byte count)
/// mov r9, r14                   ; 4D 89 F1    (arg4 = *mut FVoiceEngineImpl)
/// ```
pub const SUBMIT_REMOTE_VOICE_PROLOGUE: [u8; 24] = [
    0x48, 0x83, 0xEC, 0x20, 0x4C, 0x89, 0xE9, 0x49, 0x8B, 0x96, 0x40, 0x01, 0x00, 0x00, 0x44, 0x8B,
    0x85, 0x80, 0x00, 0x00, 0x00, 0x4D, 0x89, 0xF1,
];

/// Epilogue machine code following `mov rax, imm64` in the `SubmitRemoteVoiceData` relay stub:
/// ```text
/// call rax                      ; FF D0
/// add rsp, 0x20                 ; 48 83 C4 20
/// ```
pub const SUBMIT_REMOTE_VOICE_CALL_AND_EPILOGUE: [u8; 6] = [0xFF, 0xD0, 0x48, 0x83, 0xC4, 0x20];

/// Exact byte length of the `SubmitRemoteVoiceData` relay stub (`24 + 10 + 6 + 14 = 54`).
pub const SUBMIT_REMOTE_VOICE_STUB_LEN: usize = 54;

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

/// Total number of client patches applied at startup (`7` direct + `1` vtable + `1` mid-function
/// hook + `1` guard detour + `6` function detours = `16`).
pub const TOTAL_PATCH_COUNT: usize = DIRECT_PATCHES.len() + 9;

/// All verified direct machine-code byte patches for `DreadHunger-Win64-Shipping.exe`.
pub const DIRECT_PATCHES: [BytePatch; 7] = [
    // FVoiceModule::StartupModule (`0x3049065`): force `bEnabled = 1` when loaded early via proxy.
    BytePatch {
        name: "FVoiceModule::StartupModule force bEnabled",
        rva: 0x0304_9065,
        expected: &[0x80, 0x3B, 0x00, 0x0F, 0x84, 0xD2, 0x00, 0x00, 0x00],
        replacement: &[0xC6, 0x03, 0x01, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
    },
    // FVoiceEngineImpl::Init (`0x90CB1B`): also force `FVoiceModule::bEnabled = 1` (`[rax+0x10]`)
    // right before `CreateVoiceCapture`, supporting late runtime injection after StartupModule.
    BytePatch {
        name: "FVoiceEngineImpl::Init force FVoiceModule::bEnabled",
        rva: 0x0090_CB1B,
        expected: &[0x80, 0x78, 0x10, 0x00, 0x0F, 0x84, 0x93, 0x01, 0x00, 0x00],
        replacement: &[0xC6, 0x40, 0x10, 0x01, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
    },
    // FOnlineVoiceImpl::Init (`0x90C77B`): bypass `bHasVoiceEnabled` config gate #1.
    BytePatch {
        name: "FOnlineVoiceImpl::Init bypass bHasVoiceEnabled check #1",
        rva: 0x0090_C77B,
        expected: &[0x0F, 0x84, 0xA8, 0x02, 0x00, 0x00],
        replacement: &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
    },
    // FOnlineVoiceImpl::Init (`0x90C785`): bypass `bHasVoiceEnabled` config gate #2.
    BytePatch {
        name: "FOnlineVoiceImpl::Init bypass bHasVoiceEnabled check #2",
        rva: 0x0090_C785,
        expected: &[0x0F, 0x84, 0x9E, 0x02, 0x00, 0x00],
        replacement: &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
    },
    // FOnlineVoiceImpl::Tick (`0x9296B2`): skip `SessionInt->GetNumSessions() <= 0` gate so
    // native voice ticks even after EOS-bypass direct-IP reconnect without an active OSS session.
    BytePatch {
        name: "FOnlineVoiceImpl::Tick skip GetNumSessions gate",
        rva: 0x0092_96B2,
        expected: &[0x48, 0x8B, 0x4B, 0x28],
        replacement: &[0xEB, 0x14, 0x90, 0x90],
    },
    // UVOIPStatics::GetVoiceSampleRate (`0x2D63900`): force 48000 Hz (`0xBB80`) end-to-end
    // (DirectSoundCapture + Opus encoder/decoder -> Dread Hunger 48 kHz `USoundWaveProcedural`).
    BytePatch {
        name: "UVOIPStatics::GetVoiceSampleRate force 48000 Hz",
        rva: 0x02D6_3900,
        expected: &[0x48, 0x83, 0xEC, 0x48, 0x80, 0x3D],
        replacement: &[0xB8, 0x80, 0xBB, 0x00, 0x00, 0xC3],
    },
    // APlayerController::ClientEnableNetworkVoice_Implementation (`0x2B603E0`): ignore server's
    // automatic open-mic RPC so Dread Hunger's PTT / Toggle-to-Talk retains full control.
    BytePatch {
        name: "APlayerController::ClientEnableNetworkVoice_Impl no-op",
        rva: 0x02B6_03E0,
        expected: &[0x48, 0x8B, 0x01],
        replacement: &[0xC3, 0x90, 0x90],
    },
];

/// Vtable patch redirecting `FOnlineSubsystemEOSPlus::GetVoiceInterface` (slot 13, `+0x68`)
/// to our delegating thunk calling `FOnlineSubsystemSteam::GetVoiceInterface`.
pub const EOS_PLUS_GET_VOICE_VTABLE_PATCH: VtablePatchSpec = VtablePatchSpec {
    name: "FOnlineSubsystemEOSPlus::GetVoiceInterface vtable delegation",
    slot_rva: 0x0375_54C8,
    expected_target_rva: 0x0076_EB20,
};

/// Mid-function hook inside `FVoiceEngineImpl::SubmitRemoteVoiceData` (`0x927C0C`), immediately
/// after `FVoiceDecoderOpus::Decode` succeeds, redirecting decoded 48 kHz mono PCM to Dread
/// Hunger's `UDH_OnlineVoice::BeforeRecvAudioRendered` and jumping to clean epilogue `0x92824B`.
pub const SUBMIT_REMOTE_VOICE_HOOK: SubmitRemoteVoiceSpec = SubmitRemoteVoiceSpec {
    name: "FVoiceEngineImpl::SubmitRemoteVoiceData PCM redirect",
    hook_rva: 0x0092_7C0C,
    expected_bytes: &[
        0x48, 0x8B, 0x46, 0x20, 0x0F, 0x29, 0xB4, 0x24, 0x30, 0x01, 0x00, 0x00,
    ],
    epilogue_rva: 0x0092_824B,
};

/// Guard detour on `FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered` (`0xA2DC70`)
/// suppressing EOS RTC playback while on a Dedicated Server (prevents any echo/double audio).
pub const EOS_BEFORE_RECV_AUDIO_GUARD: EosRecvGuardSpec = EosRecvGuardSpec {
    name: "FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered match guard",
    rva: 0x00A2_DC70,
    expected_prologue: [0x48, 0x8B, 0xC4, 0x48, 0x81, 0xEC, 0x88, 0x00, 0x00, 0x00],
};

/// Detour on `UNetDriver::LoadChannelDefinitions` (`0x2A61350`).
pub const DETOUR_LOAD_CHANNEL_DEFINITIONS: DetourSpec = DetourSpec {
    name: "UNetDriver::LoadChannelDefinitions ensure Voice channel",
    rva: 0x02A6_1350,
    expected_prologue: &[0x4C, 0x8B, 0xDC, 0x55, 0x53],
};

/// Detour on `FOnlineVoiceImpl::Tick` (`0x929690`) driving the Plan A dual-mode state machine.
pub const DETOUR_ONLINE_VOICE_TICK: DetourSpec = DetourSpec {
    name: "FOnlineVoiceImpl::Tick dual-mode controller",
    rva: 0x0092_9690,
    expected_prologue: &[0x40, 0x53, 0x48, 0x83, 0xEC, 0x30],
};

/// Detour on `UDH_OnlineVoice::SetWantsToTalk` (`0xEC8FE0`).
pub const DETOUR_SET_WANTS_TO_TALK: DetourSpec = DetourSpec {
    name: "UDH_OnlineVoice::SetWantsToTalk native PTT bridge",
    rva: 0x00EC_8FE0,
    expected_prologue: &[0x40, 0x53, 0x57, 0x41, 0x56],
};

/// Detour on `UDH_OnlineVoice::GetWantsToTalk` (`0xEB1F60`).
pub const DETOUR_GET_WANTS_TO_TALK: DetourSpec = DetourSpec {
    name: "UDH_OnlineVoice::GetWantsToTalk native PTT query",
    rva: 0x00EB_1F60,
    expected_prologue: &[0x48, 0x89, 0x5C, 0x24, 0x18],
};

/// Detour on `UDH_OnlineVoice::IsParticipantSpeaking` (`0xEB4F90`).
pub const DETOUR_IS_PARTICIPANT_SPEAKING: DetourSpec = DetourSpec {
    name: "UDH_OnlineVoice::IsParticipantSpeaking native indicator",
    rva: 0x00EB_4F90,
    expected_prologue: &[0x48, 0x89, 0x5C, 0x24, 0x08],
};

/// Detour on `UDH_OnlineVoice::IsPlayerInVoiceChat` (`0xEB5140`).
pub const DETOUR_IS_PLAYER_IN_VOICE_CHAT: DetourSpec = DetourSpec {
    name: "UDH_OnlineVoice::IsPlayerInVoiceChat Tab-menu mute enabler",
    rva: 0x00EB_5140,
    expected_prologue: &[0x48, 0x89, 0x5C, 0x24, 0x10],
};
