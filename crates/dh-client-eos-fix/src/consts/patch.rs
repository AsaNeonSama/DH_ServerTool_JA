use crate::structs::patch::{
    AddLobbyAttributePrivateUriSpec, BytePatch, OnDestroyClearStartedSpec, OnJoinResetUriSpec,
    OnParticipantsKeepMembersSpec, OwnerResetTrampolineSpec, PostUpdateUriResetSpec,
    SetLobbyAttributesMaxLenSpec, SetUriStartTimeTrampolineSpec,
};

/// Target client executable basename that receives in-memory patches.
pub const TARGET_PROCESS_NAME: &str = "DreadHunger-Win64-Shipping.exe";

/// Number of direct in-place EOS lobby code patches applied to the client image.
pub const DIRECT_PATCH_COUNT: usize = 11;

/// Number of owner-gated `ResetURI` trampoline patches (`OnMatchEnded`, `ResetCurrentData`).
pub const OWNER_RESET_TRAMPOLINE_COUNT: usize = 2;

/// Number of reconnect & session-hygiene trampoline patches (`SetLobbyAttributes`,
/// `OnDestroySessionComplete`, `OnJoinSessionComplete`, `OnSessionParticipantsChange`).
pub const RECONNECT_TRAMPOLINE_COUNT: usize = 4;

/// Number of optional `private-uri` (Plan B) trampoline patches enabled in this build.
#[cfg(feature = "private-uri")]
pub const PRIVATE_URI_PATCH_COUNT: usize = 1;

/// Number of optional `private-uri` (Plan B) trampoline patches enabled in this build.
#[cfg(not(feature = "private-uri"))]
pub const PRIVATE_URI_PATCH_COUNT: usize = 0;

/// Total number of patches enabled in this build (Plan A: 19 patches; Plan B `private-uri`: 20 patches).
pub const TOTAL_PATCH_COUNT: usize = DIRECT_PATCH_COUNT
    + 1
    + OWNER_RESET_TRAMPOLINE_COUNT
    + 1
    + RECONNECT_TRAMPOLINE_COUNT
    + PRIVATE_URI_PATCH_COUNT;

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

/// Size in bytes of the owner-gated `ResetURI` + `bHasGameStarted` reset trampoline stub
/// (`OnMatchEnded` / `ResetCurrentData`).
pub const OWNER_RESET_TRAMPOLINE_LEN: usize = 53;

/// Size in bytes of the `SetURI` `StartUTCDateTime` trampoline stub (`0xE7E386`).
pub const SET_URI_START_TIME_TRAMPOLINE_LEN: usize = 91;

/// Size in bytes of the Plan B `AddLobbyAttribute` private-URI trampoline stub (`0x9CA894`).
pub const ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN: usize = 75;

/// Byte length of the original `strncpy(..., 0x101)` sequence in
/// `FOnlineSessionEOS::SetLobbyAttributes` (`0xA03820..0xA0383A`).
pub const SET_LOBBY_ATTR_SITE_LEN: usize = 26;

/// EOS SDK maximum lobby attribute string value length (`EOS_LOBBY_MAX_KEY_LENGTH` / value limit =
/// 1000 UTF-8 bytes excluding NUL).
pub const EOS_LOBBY_MAX_ATTRIBUTE_LEN: usize = 1000;

/// Size in bytes of the `SetLobbyAttributes` UTF-8 safe 1000-byte copy trampoline (`0xA03820`).
pub const SET_LOBBY_ATTR_MAX_LEN_TRAMPOLINE_LEN: usize = 98;

/// Size in bytes of the `OnDestroySessionComplete` `bHasGameStarted` clear trampoline (`0xE7EDC9`).
pub const ON_DESTROY_CLEAR_STARTED_TRAMPOLINE_LEN: usize = 17;

/// Size in bytes of the `OnJoinSessionComplete` `ResetURI` trampoline (`0xE815D7`).
pub const ON_JOIN_RESET_URI_TRAMPOLINE_LEN: usize = 41;

/// Size in bytes of the `OnSessionParticipantsChange` `IsLobbyMap` guard trampoline (`0xE85B28`).
pub const ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE_LEN: usize = 20;

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

/// x86-64 `jmp rel32` opcode (`0xE9`).
pub const X64_JMP_REL32_OPCODE: u8 = 0xE9;

/// Length in bytes of an x86-64 `jmp rel32` instruction.
pub const X64_JMP_REL32_LEN: usize = 5;

/// Length in bytes of an x86-64 RIP-relative indirect `call qword ptr [rip + rel32]` (`FF 15`)
/// or `jmp qword ptr [rip + rel32]` (`FF 25`) instruction.
pub const X64_CALL_MEM_REL32_LEN: usize = 6;

/// x86-64 `jmp qword ptr [rip + rel32]` opcode prefix (`FF 25`).
pub const X64_JMP_MEM_REL32_OPCODES: [u8; 2] = [0xFF, 0x25];

/// x86-64 `mov rcx, imm64` opcode prefix (`48 B9`).
pub const X64_MOV_RCX_IMM64_PREFIX: [u8; 2] = [0x48, 0xB9];

/// x86-64 single-byte `nop` opcode (`0x90`), used to pad wider patch sites after writing a
/// 5-byte `call rel32`.
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

/// `test rdi, rdi; je +7; mov byte ptr [rdi+0x16E], 0; sub rsp, 0x28` — clears
/// `UDH_OnlineSession::bHasGameStarted` (`+0x16E`) for both host and non-host on match end /
/// lobby reset (`rdi == this`), then aligns stack to 16 bytes with 32-byte Win64 shadow space.
pub const OWNER_RESET_PROLOGUE: [u8; 16] = [
    0x48, 0x85, 0xFF, // test rdi, rdi
    0x74, 0x07, // je +7
    0xC6, 0x87, 0x6E, 0x01, 0x00, 0x00, 0x00, // mov byte ptr [rdi+0x16E], 0
    0x48, 0x83, 0xEC, 0x28, // sub rsp, 0x28
];

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

/// Body of the `SetLobbyAttributes_MembersMax1000` trampoline following `mov rcx, <rw_buf>`
/// (`0x0A..0x62`): stores `rcx` into `EOS_Lobby_AttributeData::Value.AsUtf8` (`[rbp+0x80]`),
/// copies up to `1000` (`0x3E8`) bytes from `[rax+0x88]` (`FTCHARToUTF8::Get()`) without
/// splitting any multi-byte UTF-8 codepoint, writes a trailing NUL byte, returns `rcx` in `rax`,
/// and returns directly to `0xA0383A`. Uses only volatile registers (`rax`, `rcx`, `rdx`, `r8`).
pub const SET_LOBBY_ATTR_MAX_LEN_BODY: [u8; 88] = [
    0x48, 0x89, 0x8D, 0x80, 0x00, 0x00, 0x00, // 0x0A: mov qword ptr [rbp+0x80], rcx
    0x45, 0x31, 0xC0, // 0x11: xor r8d, r8d
    0x48, 0x85, 0xC0, // 0x14: test rax, rax
    0x74, 0x40, // 0x17: jz +0x40 (-> 0x59 write_nul)
    0x48, 0x8B, 0x90, 0x88, 0x00, 0x00, 0x00, // 0x19: mov rdx, qword ptr [rax+0x88]
    0x48, 0x85, 0xD2, // 0x20: test rdx, rdx
    0x74, 0x34, // 0x23: jz +0x34 (-> 0x59 write_nul)
    0x42, 0x8A, 0x04, 0x02, // 0x25: copy_loop: mov al, byte ptr [rdx+r8]
    0x84, 0xC0, // 0x29: test al, al
    0x74, 0x2C, // 0x2B: jz +0x2C (-> 0x59 write_nul)
    0x42, 0x88, 0x04, 0x01, // 0x2D: mov byte ptr [rcx+r8], al
    0x49, 0xFF, 0xC0, // 0x31: inc r8
    0x49, 0x81, 0xF8, 0xE8, 0x03, 0x00, 0x00, // 0x34: cmp r8, 1000 (0x3E8)
    0x72, 0xE8, // 0x3B: jb -24 (-> 0x25 copy_loop)
    0x42, 0x8A, 0x04, 0x02, // 0x3D: mov al, byte ptr [rdx+r8]
    0x24, 0xC0, // 0x41: and al, 0xC0
    0x3C, 0x80, // 0x43: cmp al, 0x80
    0x75, 0x12, // 0x45: jne +0x12 (-> 0x59 write_nul)
    0x4D, 0x85, 0xC0, // 0x47: back_loop: test r8, r8
    0x74, 0x0D, // 0x4A: jz +0x0D (-> 0x59 write_nul)
    0x49, 0xFF, 0xC8, // 0x4C: dec r8
    0x42, 0x8A, 0x04, 0x01, // 0x4F: mov al, byte ptr [rcx+r8]
    0x24, 0xC0, // 0x53: and al, 0xC0
    0x3C, 0x80, // 0x55: cmp al, 0x80
    0x74, 0xEE, // 0x57: je -18 (-> 0x47 back_loop)
    0x42, 0xC6, 0x04, 0x01, 0x00, // 0x59: write_nul: mov byte ptr [rcx+r8], 0
    0x48, 0x89, 0xC8, // 0x5E: mov rax, rcx
    0xC3, // 0x61: ret
];

/// `test rcx, rcx; je +7; mov byte ptr [rcx+0x16E], 0` — clears
/// `UDH_OnlineSession::bHasGameStarted` (`+0x16E`) inside `OnDestroySessionComplete` (`0xE7EDC9`)
/// before tail-jumping to `0xF072B0`.
pub const ON_DESTROY_CLEAR_STARTED_PREFIX: [u8; 12] = [
    0x48, 0x85, 0xC9, // test rcx, rcx
    0x74, 0x07, // je +7
    0xC6, 0x81, 0x6E, 0x01, 0x00, 0x00, 0x00, // mov byte ptr [rcx+0x16E], 0
];

/// `sub rsp, 0x28; mov [rsp+0x20], rcx` — aligns stack and preserves `UDH_OnlineSession* this`
/// across `GetInstance` / `ResetURI` inside `OnJoinSessionComplete` (`0xE815D7`).
pub const ON_JOIN_RESET_URI_PROLOGUE: [u8; 9] = [
    0x48, 0x83, 0xEC, 0x28, // sub rsp, 0x28
    0x48, 0x89, 0x4C, 0x24, 0x20, // mov qword ptr [rsp+0x20], rcx
];

/// `mov rcx, [rsp+0x20]; add rsp, 0x28` — restores `UDH_OnlineSession* this` and original stack
/// before tail-jumping to `UDH_OnlineSession::Update`.
pub const ON_JOIN_RESET_URI_RESTORE: [u8; 9] = [
    0x48, 0x8B, 0x4C, 0x24, 0x20, // mov rcx, qword ptr [rsp+0x20]
    0x48, 0x83, 0xC4, 0x28, // add rsp, 0x28
];

/// Checks `UDH_OnlineSession::StartUTCDateTime` (`[rcx+0x48]`): if non-zero (match already
/// started), returns `false` (`al = 0`) so `OnSessionParticipantsChange` does not erase leaving
/// players from `MEMBERS`; otherwise falls through to `jmp rel32 IsLobbyMap`.
pub const ON_PARTICIPANTS_KEEP_MEMBERS_CHECK: [u8; 15] = [
    0x48, 0x85, 0xC9, // 0x00: test rcx, rcx
    0x74, 0x0A, // 0x03: je +0x0A (-> 0x0F jmp IsLobbyMap)
    0x48, 0x83, 0x79, 0x48, 0x00, // 0x05: cmp qword ptr [rcx+0x48], 0
    0x74, 0x03, // 0x0A: je +0x03 (-> 0x0F jmp IsLobbyMap)
    0x31, 0xC0, // 0x0C: xor eax, eax
    0xC3, // 0x0E: ret
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

/// Client RVA of the `GetGameInstance<UDH_GameInstance>(const UObject*)` helper called inside
/// `UDH_OnlineSession::OnDestroySessionComplete` (`0xE7EDC9`).
pub const CLIENT_DESTROY_GET_GAME_INSTANCE_RVA: usize = 0x00F0_72B0;

/// Client RVA of `IsLobbyMap(const UObject*)` called inside
/// `UDH_OnlineSession::OnSessionParticipantsChange` (`0xE85B28`).
pub const CLIENT_IS_LOBBY_MAP_RVA: usize = 0x00F0_AF50;

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
    BytePatch {
        name: "SteamUpdateLobbyTask_AllowStartedLobbyType",
        rva: 0x0098_2DEC,
        expected: &[0x75, 0x17],
        replacement: &[0x90, 0x90],
    },
    BytePatch {
        name: "UpdatePresence_AllowStartedConnectLobby_StartTime",
        rva: 0x00E9_52E5,
        expected: &[0x0F, 0x85, 0xF0, 0x00, 0x00, 0x00],
        replacement: &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
    },
    BytePatch {
        name: "UpdatePresence_AllowStartedConnectLobby_ListenReady",
        rva: 0x00E9_52F0,
        expected: &[0x0F, 0x84, 0xE5, 0x00, 0x00, 0x00],
        replacement: &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
    },
    BytePatch {
        name: "Search_IncludeInProgressLobbies",
        rva: 0x00E8_F0E2,
        expected: &[0x75, 0x41],
        replacement: &[0xEB, 0x41],
    },
    BytePatch {
        name: "OnMatchEnded_KeepPublic",
        rva: 0x00E8_1B22,
        expected: &[0x40, 0x88, 0x77, 0x40],
        replacement: &[0x90, 0x90, 0x90, 0x90],
    },
    BytePatch {
        name: "ResetCurrentDataInner_KeepPublic",
        rva: 0x00E8_B17B,
        expected: &[0xC6, 0x01, 0x00],
        replacement: &[0x90, 0x90, 0x90],
    },
    BytePatch {
        name: "ResetCurrentData_KeepLobbyPublic",
        rva: 0x00E8_B338,
        expected: &[0xC6, 0x80, 0xB0, 0x04, 0x00, 0x00, 0x00],
        replacement: &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
    },
    BytePatch {
        name: "OnOnlineSessionUpdateComplete_KeepLobbyPublic",
        rva: 0x00E8_2734,
        expected: &[0xC6, 0x80, 0xB0, 0x04, 0x00, 0x00, 0x00],
        replacement: &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
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

/// Replaces the 26-byte `strncpy(..., 0x101)` sequence in `FOnlineSessionEOS::SetLobbyAttributes`
/// (`0xA03820..0xA0383A`) so string lobby attributes (`MEMBERS` for up to 8 players) are copied
/// up to `1000` UTF-8 bytes without truncating at `256` bytes or splitting multi-byte UTF-8
/// characters.
pub const SET_LOBBY_ATTRIBUTES_MAX_LEN_TRAMPOLINE: SetLobbyAttributesMaxLenSpec =
    SetLobbyAttributesMaxLenSpec {
        name: "SetLobbyAttributes_MembersMax1000",
        site_rva: 0x00A0_3820,
        expected_site_bytes: [
            0x41, 0xB8, 0x01, 0x01, 0x00, 0x00, // mov r8d, 0x101
            0x48, 0x8D, 0x8D, 0x91, 0x01, 0x00, 0x00, // lea rcx, [rbp+0x191]
            0x48, 0x8B, 0x90, 0x88, 0x00, 0x00, 0x00, // mov rdx, [rax+0x88]
            0xFF, 0x15, 0x6E, 0xB3, 0xC6, 0x02, // call qword ptr [IAT strncpy]
        ],
    };

/// Detours `0xE7EDC9` (`call 0xF072B0`) inside `UDH_OnlineSession::OnDestroySessionComplete` so
/// stale `UDH_OnlineSession::bHasGameStarted` (`+0x16E`) is cleared to `0` on session teardown.
pub const ON_DESTROY_CLEAR_STARTED_TRAMPOLINE: OnDestroyClearStartedSpec =
    OnDestroyClearStartedSpec {
        name: "OnDestroyComplete_ClearHasGameStarted",
        call_site_rva: 0x00E7_EDC9,
        expected_call_bytes: [0xE8, 0xE2, 0x84, 0x08, 0x00],
        original_callee_rva: CLIENT_DESTROY_GET_GAME_INSTANCE_RVA,
    };

/// Detours `UDH_OnlineSession::Update()` (`0xE95760`) inside
/// `UDH_OnlineSession::OnJoinSessionComplete` (`0xE815D7`) so reconnecting players clear stale
/// `UDH_GameInstance + 0x570` (`ResetURI`) once when joining a session before `PostUpdate`
/// evaluates `CanTravelToURI`.
pub const ON_JOIN_RESET_URI_TRAMPOLINE: OnJoinResetUriSpec = OnJoinResetUriSpec {
    name: "OnJoinSessionComplete_ResetUri",
    call_site_rva: 0x00E8_15D7,
    expected_call_bytes: [0xE8, 0x84, 0x41, 0x01, 0x00],
    get_game_instance_rva: CLIENT_GET_GAME_INSTANCE_RVA,
    reset_uri_rva: CLIENT_RESET_URI_RVA,
    update_session_rva: CLIENT_ONLINE_SESSION_UPDATE_RVA,
};

/// Detours `IsLobbyMap` (`0xF0AF50`) inside `UDH_OnlineSession::OnSessionParticipantsChange`
/// (`0xE85B28`) so leaving crewmates remain in the EOS `MEMBERS` list once `StartUTCDateTime`
/// (`+0x48`) is non-zero, enabling in-progress reconnect via `TestCanJoin`.
pub const ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE: OnParticipantsKeepMembersSpec =
    OnParticipantsKeepMembersSpec {
        name: "OnSessionParticipantsChange_KeepMembersAfterStart",
        call_site_rva: 0x00E8_5B28,
        expected_call_bytes: [0xE8, 0x23, 0x54, 0x08, 0x00],
        is_lobby_map_rva: CLIENT_IS_LOBBY_MAP_RVA,
    };
