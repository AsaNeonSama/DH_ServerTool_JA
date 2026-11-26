use crate::structs::patch::{
    AddLobbyAttributePrivateUriSpec, BytePatch, OwnerResetTrampolineSpec, PostUpdateUriResetSpec,
    SetUriStartTimeTrampolineSpec,
};

/// Target client executable basename that receives in-memory patches.
pub const TARGET_PROCESS_NAME: &str = "DreadHunger-Win64-Shipping.exe";

/// Number of direct in-place EOS lobby code patches applied to the client image.
pub const DIRECT_PATCH_COUNT: usize = 3;

/// Number of owner-gated `ResetURI` trampoline patches (`OnMatchEnded`, `ResetCurrentData`).
pub const OWNER_RESET_TRAMPOLINE_COUNT: usize = 2;

/// Number of optional `private-uri` (Plan B) trampoline patches enabled in this build.
#[cfg(feature = "private-uri")]
pub const PRIVATE_URI_PATCH_COUNT: usize = 1;

/// Number of optional `private-uri` (Plan B) trampoline patches enabled in this build.
#[cfg(not(feature = "private-uri"))]
pub const PRIVATE_URI_PATCH_COUNT: usize = 0;

/// Total number of patches enabled in this build (Plan A: 7 patches; Plan B `private-uri`: 8 patches).
pub const TOTAL_PATCH_COUNT: usize =
    DIRECT_PATCH_COUNT + 1 + OWNER_RESET_TRAMPOLINE_COUNT + 1 + PRIVATE_URI_PATCH_COUNT;

/// Expected DOS magic (`MZ`) at offset `0x00`.
pub const PE_DOS_MAGIC: u16 = 0x5A4D;

/// Offset of `e_lfanew` inside `IMAGE_DOS_HEADER`.
pub const PE_LFANEW_OFFSET: usize = 0x3C;

/// Expected NT signature (`PE\0\0`).
pub const PE_NT_SIGNATURE: u32 = 0x0000_4550;

/// Offset of `IMAGE_OPTIONAL_HEADER64` relative to the NT headers base (`Signature` + `FileHeader`).
pub const PE_OPTIONAL_HEADER_OFFSET: usize = 0x18;

/// Expected magic value of `IMAGE_OPTIONAL_HEADER64` (`PE32+`).
pub const PE_OPT_MAGIC_PE32_PLUS: u16 = 0x020B;

/// Offset of `SizeOfImage` relative to the start of `IMAGE_OPTIONAL_HEADER64`.
pub const PE_SIZE_OF_IMAGE_OFFSET: usize = 0x38;

/// Minimum byte span needed to inspect the DOS header and `e_lfanew`.
pub const PE_MIN_DOS_BYTES: usize = 0x40;

/// Minimum byte span needed inside the NT header to read `SizeOfImage`.
pub const PE_MIN_NT_BYTES: usize = PE_OPTIONAL_HEADER_OFFSET + PE_SIZE_OF_IMAGE_OFFSET + 4;

/// Size in bytes of the `PostUpdate` empty-URI conditional `ResetURI` trampoline stub.
pub const POST_UPDATE_TRAMPOLINE_LEN: usize = 61;

/// Size in bytes of the owner-gated `ResetURI` trampoline stub (`OnMatchEnded` / `ResetCurrentData`).
pub const OWNER_RESET_TRAMPOLINE_LEN: usize = 41;

/// Size in bytes of the `SetURI` `StartUTCDateTime` trampoline stub (`0xE7E386`).
pub const SET_URI_START_TIME_TRAMPOLINE_LEN: usize = 91;

/// Size in bytes of the Plan B `AddLobbyAttribute` private-URI trampoline stub (`0x9CA894`).
pub const ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN: usize = 75;

/// Page size requested when allocating a nearby executable trampoline.
pub const TRAMPOLINE_ALLOC_SIZE: usize = 0x1000;

/// Maximum distance from module base (1.5 GiB) when searching for a nearby ±2 GiB trampoline slot.
pub const TRAMPOLINE_MAX_DISTANCE: usize = 0x6000_0000;

/// Fallback allocation granularity (64 KiB) if `GetSystemInfo` returns zero.
pub const DEFAULT_ALLOC_GRANULARITY: usize = 0x10000;

/// x86-64 `call rel32` opcode (`0xE8`).
pub const X64_CALL_REL32_OPCODE: u8 = 0xE8;

/// Length in bytes of an x86-64 `call rel32` instruction.
pub const X64_CALL_REL32_LEN: usize = 5;

/// Length in bytes of an x86-64 RIP-relative indirect `call qword ptr [rip + rel32]` (`FF 15`)
/// or `jmp qword ptr [rip + rel32]` (`FF 25`) instruction.
pub const X64_CALL_MEM_REL32_LEN: usize = 6;

/// x86-64 `jmp qword ptr [rip + rel32]` opcode prefix (`FF 25`).
pub const X64_JMP_MEM_REL32_OPCODES: [u8; 2] = [0xFF, 0x25];

/// x86-64 single-byte `nop` opcode (`0x90`), used to pad a 6-byte `call [rip+rel32]` site after
/// writing a 5-byte `call rel32`.
pub const X64_NOP_BYTE: u8 = 0x90;

/// `int 3` padding byte written to unused bytes of each trampoline page.
pub const X64_INT3_BYTE: u8 = 0xCC;

/// `push r8; sub rsp, 0x20` — preserves `&SessionURI` and reserves aligned 32-byte shadow space.
pub const POST_UPDATE_PROLOGUE: [u8; 6] = [0x41, 0x50, 0x48, 0x83, 0xEC, 0x20];

/// `mov rcx, [rsp+0x20]; test al, al; je +6; cmp dword ptr [rcx+8], 1; jg +30`
pub const POST_UPDATE_CHECK_EMPTY: [u8; 15] = [
    0x48, 0x8B, 0x4C, 0x24, 0x20, 0x84, 0xC0, 0x74, 0x06, 0x83, 0x79, 0x08, 0x01, 0x7F, 0x1E,
];

/// `mov [rsp+0x20], al; mov rcx, rsi` — preserves `al` and loads `UDH_OnlineSession* this` (`rsi`).
pub const POST_UPDATE_PREP_GET_GI: [u8; 7] = [0x88, 0x44, 0x24, 0x20, 0x48, 0x89, 0xF1];

/// `test rax, rax; je +8; mov rcx, rax` — guards null `UDH_GameInstance*` before `ResetURI`.
pub const TRAMPOLINE_CHECK_GI_NONNULL: [u8; 8] = [0x48, 0x85, 0xC0, 0x74, 0x08, 0x48, 0x89, 0xC1];

/// `movzx eax, byte ptr [rsp+0x20]; add rsp, 0x28; ret`
pub const POST_UPDATE_EPILOGUE: [u8; 10] =
    [0x0F, 0xB6, 0x44, 0x24, 0x20, 0x48, 0x83, 0xC4, 0x28, 0xC3];

/// `sub rsp, 0x28` — aligns stack to 16 bytes and reserves 32-byte Win64 shadow space.
pub const OWNER_RESET_PROLOGUE: [u8; 4] = [0x48, 0x83, 0xEC, 0x28];

/// `test al, al; je +23; mov rcx, rdi` — skips reset if non-owner, else loads `this` (`rdi`).
pub const OWNER_RESET_CHECK_OWNER: [u8; 7] = [0x84, 0xC0, 0x74, 0x17, 0x48, 0x89, 0xF9];

/// `mov al, 1; add rsp, 0x28; ret` — restores `IsSessionOwner() == true` and returns.
pub const OWNER_RESET_EPILOGUE: [u8; 7] = [0xB0, 0x01, 0x48, 0x83, 0xC4, 0x28, 0xC3];

/// `sub rsp, 0x38; mov [rsp+0x28], rcx; mov qword ptr [rsp+0x20], 0; lea rcx, [rsp+0x20]` —
/// aligns stack to 16 bytes with 32-byte Win64 shadow space, saves `UDH_OnlineSession* this`,
/// zero-initializes local `FDateTime`, and loads `rcx = &out_ticks` for `FDateTime::UtcNow`.
pub const SET_URI_START_TIME_PROLOGUE: [u8; 23] = [
    0x48, 0x83, 0xEC, 0x38, 0x48, 0x89, 0x4C, 0x24, 0x28, 0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00,
    0x00, 0x00, 0x48, 0x8D, 0x4C, 0x24, 0x20,
];

/// Converts `FDateTime::Ticks` at `[rsp+0x20]` to Unix UTC seconds
/// `(ticks - 0x089F7FF5F7B58000) / 10_000_000` (clamping `<= 0` to `1`), restores `rcx = this`,
/// and stores the timestamp into `[rcx + 0x48]` (`UDH_OnlineSession::StartUTCDateTime`).
pub const SET_URI_START_TIME_CONVERT_AND_STORE: [u8; 53] = [
    0x48, 0x8B, 0x44, 0x24, 0x20, // mov rax, [rsp+0x20]
    0x49, 0xB8, 0x00, 0x80, 0xB5, 0xF7, 0xF5, 0x7F, 0x9F, 0x08, // mov r8, 621355968000000000
    0x4C, 0x39, 0xC0, // cmp rax, r8
    0x7E, 0x13, // jle +0x13 (clamp_one)
    0x4C, 0x29, 0xC0, // sub rax, r8
    0x31, 0xD2, // xor edx, edx
    0x41, 0xB8, 0x80, 0x96, 0x98, 0x00, // mov r8d, 10_000_000
    0x49, 0xF7, 0xF0, // div r8
    0x48, 0x85, 0xC0, // test rax, rax
    0x75, 0x05, // jne +0x05 (store)
    0xB8, 0x01, 0x00, 0x00, 0x00, // clamp_one: mov eax, 1
    0x48, 0x8B, 0x4C, 0x24, 0x28, // store: mov rcx, [rsp+0x28]
    0x48, 0x89, 0x41, 0x48, // mov [rcx+0x48], rax
];

/// `add rsp, 0x38; ret` — restores stack frame and returns to `UDH_OnlineSession::SetURI`.
pub const SET_URI_START_TIME_EPILOGUE: [u8; 5] = [0x48, 0x83, 0xC4, 0x38, 0xC3];

/// Inspects `EOS_LobbyModification_AddAttributeOptions*` (`rdx`) -> `Attribute` (`[rdx+0x08]`) ->
/// `Key` (`[attr+0x08]`) byte-by-byte without reading past NUL. When `Key` matches ASCII `"URI"`
/// case-insensitively (`'U'|'u'`, `'R'|'r'`, `'I'|'i'`, `'\0'`), writes `Visibility = 1`
/// (`EOS_LAT_PRIVATE`) at `[rdx+0x10]`; otherwise leaves `Visibility = 0` (`EOS_LAT_PUBLIC`).
/// Preserves `rcx`, `rdx`, `rsp`, and all callee-saved registers before tail-jumping at `+0x45`.
pub const ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_CHECK: [u8; 69] = [
    0x48, 0x85, 0xD2, // 0x00: test rdx, rdx
    0x74, 0x40, // 0x03: je +0x40 (-> 0x45)
    0x4C, 0x8B, 0x42, 0x08, // 0x05: mov r8, [rdx+0x08]
    0x4D, 0x85, 0xC0, // 0x09: test r8, r8
    0x74, 0x37, // 0x0C: je +0x37 (-> 0x45)
    0x4D, 0x8B, 0x40, 0x08, // 0x0E: mov r8, [r8+0x08]
    0x4D, 0x85, 0xC0, // 0x12: test r8, r8
    0x74, 0x2E, // 0x15: je +0x2E (-> 0x45)
    0x41, 0x0F, 0xB6, 0x00, // 0x17: movzx eax, byte ptr [r8]
    0x0C, 0x20, // 0x1B: or al, 0x20
    0x3C, 0x75, // 0x1D: cmp al, 'u'
    0x75, 0x24, // 0x1F: jne +0x24 (-> 0x45)
    0x41, 0x0F, 0xB6, 0x40, 0x01, // 0x21: movzx eax, byte ptr [r8+1]
    0x0C, 0x20, // 0x26: or al, 0x20
    0x3C, 0x72, // 0x28: cmp al, 'r'
    0x75, 0x19, // 0x2A: jne +0x19 (-> 0x45)
    0x41, 0x0F, 0xB6, 0x40, 0x02, // 0x2C: movzx eax, byte ptr [r8+2]
    0x0C, 0x20, // 0x31: or al, 0x20
    0x3C, 0x69, // 0x33: cmp al, 'i'
    0x75, 0x0E, // 0x35: jne +0x0E (-> 0x45)
    0x41, 0x80, 0x78, 0x03, 0x00, // 0x37: cmp byte ptr [r8+3], 0
    0x75, 0x07, // 0x3C: jne +0x07 (-> 0x45)
    0xC7, 0x42, 0x10, 0x01, 0x00, 0x00, 0x00, // 0x3E: mov dword ptr [rdx+0x10], 1
];

/// Client RVA of `UDH_GameInstance::GetInstance(const UObject*)`.
pub const CLIENT_GET_GAME_INSTANCE_RVA: usize = 0x00E0_8E20;

/// Client RVA of `UDH_GameInstance::ResetURI()`.
pub const CLIENT_RESET_URI_RVA: usize = 0x00E1_CBD0;

/// Client RVA of `UDH_OnlineSession::IsSessionOwner()`.
pub const CLIENT_IS_SESSION_OWNER_RVA: usize = 0x00E7_B3D0;

/// Client RVA of `FDateTime::UtcNow(FDateTime* out)`.
pub const CLIENT_FDATETIME_UTC_NOW_RVA: usize = 0x0119_3830;

/// Client RVA of `UDH_OnlineSession::Update()`.
pub const CLIENT_ONLINE_SESSION_UPDATE_RVA: usize = 0x00E9_5760;

/// Client RVA of the `EOS_LobbyModification_AddAttribute` IAT pointer slot.
pub const CLIENT_EOS_ADD_LOBBY_ATTRIBUTE_IAT_RVA: usize = 0x04A6_A5E0;

/// Direct in-place EOS patches verified against vanilla `DreadHunger-Win64-Shipping.exe`.
///
/// NOTE: `UDH_GameInstance::ConnectToURI` (`0xE006E0`) and `UDH_OnlineSession::CanTravelToURI`
/// (`0xE6ED90`) are intentionally kept vanilla so `UDH_GameInstance + 0x570` deduplication stays
/// intact during initial join and active matches (preventing EOS lobby updates from looping
/// `SetClientTravel`).
pub const DIRECT_PATCHES: [BytePatch; DIRECT_PATCH_COUNT] = [
    BytePatch {
        name: "CreateLobbySession_PermissionLevel",
        rva: 0x009D_691E,
        expected: &[0x0F, 0x9E, 0xC0],
        replacement: &[0x30, 0xC0, 0x90],
    },
    BytePatch {
        name: "UpdateLobbySession_PermissionLevel",
        rva: 0x00A0_9401,
        expected: &[0x0F, 0x9E, 0xC0],
        replacement: &[0x30, 0xC0, 0x90],
    },
    BytePatch {
        name: "SetLobbyAttributes_LobbySearch",
        rva: 0x00A0_333A,
        expected: &[0xC7, 0x85, 0xE0, 0x04, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00],
        replacement: &[0x0F, 0xB6, 0x47, 0x38, 0x89, 0x85, 0xE0, 0x04, 0x00, 0x00],
    },
];

/// Detours `FOnlineSessionSettings::Get<FString>(SessionSettingURIName, SessionURI)` inside
/// `UDH_OnlineSession::PostUpdate` (`0xE8894B`) to call `UDH_GameInstance::ResetURI()` whenever
/// the EOS lobby reports no active game-server URI (empty/unset).
pub const POST_UPDATE_EMPTY_URI_RESET: PostUpdateUriResetSpec = PostUpdateUriResetSpec {
    name: "PostUpdate_ResetUriWhenSessionUriEmpty",
    call_site_rva: 0x00E8_894B,
    expected_call_bytes: [0xE8, 0x70, 0x89, 0xA4, 0xFF],
    original_get_rva: 0x008D_12C0,
    get_game_instance_rva: CLIENT_GET_GAME_INSTANCE_RVA,
    reset_uri_rva: CLIENT_RESET_URI_RVA,
};

/// Owner-gated `ResetURI` call sites (`OnMatchEnded` and `ResetCurrentData`) that clear the host's
/// `UDH_GameInstance + 0x570` immediately when the lobby resets between matches.
pub const OWNER_RESET_TRAMPOLINES: [OwnerResetTrampolineSpec; OWNER_RESET_TRAMPOLINE_COUNT] = [
    OwnerResetTrampolineSpec {
        name: "OnMatchEnded_OwnerResetUri",
        call_site_rva: 0x00E8_1B59,
        expected_call_bytes: [0xE8, 0x72, 0x98, 0xFF, 0xFF],
        is_session_owner_rva: CLIENT_IS_SESSION_OWNER_RVA,
        get_game_instance_rva: CLIENT_GET_GAME_INSTANCE_RVA,
        reset_uri_rva: CLIENT_RESET_URI_RVA,
    },
    OwnerResetTrampolineSpec {
        name: "ResetCurrentData_OwnerResetUri",
        call_site_rva: 0x00E8_B3A3,
        expected_call_bytes: [0xE8, 0x28, 0x00, 0xFF, 0xFF],
        is_session_owner_rva: CLIENT_IS_SESSION_OWNER_RVA,
        get_game_instance_rva: CLIENT_GET_GAME_INSTANCE_RVA,
        reset_uri_rva: CLIENT_RESET_URI_RVA,
    },
];

/// Detours `UDH_OnlineSession::Update()` inside `UDH_OnlineSession::SetURI()` (`0xE7E386`) so
/// `ConnectToIP` stamps `UDH_OnlineSession::StartUTCDateTime` (`+0x48`) before publishing the
/// dedicated-server `URI` to EOS. Without `STARTUTCDATETIME != 0`, `CopyLobbyAttributes` leaves
/// `bHasGameStarted == 0` and `TestCanJoin` (`0xE69A2D`) falls into the pre-match lobby branch,
/// rejecting reconnecting players with `5 (ListenServerNotReady)` once the host leaves the lobby.
pub const SET_URI_START_TIME_TRAMPOLINE: SetUriStartTimeTrampolineSpec =
    SetUriStartTimeTrampolineSpec {
        name: "SetURI_SetStartUTCDateTime",
        call_site_rva: 0x00E7_E386,
        expected_call_bytes: [0xE8, 0xD5, 0x73, 0x01, 0x00],
        utc_now_rva: CLIENT_FDATETIME_UTC_NOW_RVA,
        update_session_rva: CLIENT_ONLINE_SESSION_UPDATE_RVA,
    };

/// Detours `call qword ptr [EOS_LobbyModification_AddAttribute]` inside
/// `FOnlineSessionEOS::AddLobbyAttribute` (`0x9CA894`) so Plan B (`feature = "private-uri"`)
/// switches `EOS_LobbyModification_AddAttributeOptions::Visibility` from `0 (EOS_LAT_PUBLIC)` to
/// `1 (EOS_LAT_PRIVATE)` exclusively when `Attribute->Key` is `"URI"`.
pub const ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE: AddLobbyAttributePrivateUriSpec =
    AddLobbyAttributePrivateUriSpec {
        name: "AddLobbyAttribute_PrivateURI",
        call_site_rva: 0x009C_A894,
        expected_call_bytes: [0xFF, 0x15, 0x46, 0xFD, 0x09, 0x04],
        iat_slot_rva: CLIENT_EOS_ADD_LOBBY_ATTRIBUTE_IAT_RVA,
    };
