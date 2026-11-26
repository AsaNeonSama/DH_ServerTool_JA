#[cfg(all(windows, feature = "private-uri"))]
use crate::consts::patch::ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE;
use crate::consts::patch::{
    ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_CHECK, ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN,
    DEFAULT_ALLOC_GRANULARITY, DIRECT_PATCHES, OWNER_RESET_CHECK_OWNER, OWNER_RESET_EPILOGUE,
    OWNER_RESET_PROLOGUE, OWNER_RESET_TRAMPOLINES, OWNER_RESET_TRAMPOLINE_LEN, PE_DOS_MAGIC,
    PE_LFANEW_OFFSET, PE_MIN_DOS_BYTES, PE_MIN_NT_BYTES, PE_NT_SIGNATURE,
    PE_OPTIONAL_HEADER_OFFSET, PE_OPT_MAGIC_PE32_PLUS, PE_SIZE_OF_IMAGE_OFFSET,
    POST_UPDATE_CHECK_EMPTY, POST_UPDATE_EMPTY_URI_RESET, POST_UPDATE_EPILOGUE,
    POST_UPDATE_PREP_GET_GI, POST_UPDATE_PROLOGUE, POST_UPDATE_TRAMPOLINE_LEN,
    SET_URI_START_TIME_CONVERT_AND_STORE, SET_URI_START_TIME_EPILOGUE, SET_URI_START_TIME_PROLOGUE,
    SET_URI_START_TIME_TRAMPOLINE, SET_URI_START_TIME_TRAMPOLINE_LEN, TARGET_PROCESS_NAME,
    TOTAL_PATCH_COUNT, TRAMPOLINE_ALLOC_SIZE, TRAMPOLINE_CHECK_GI_NONNULL, TRAMPOLINE_MAX_DISTANCE,
    X64_CALL_MEM_REL32_LEN, X64_CALL_REL32_LEN, X64_CALL_REL32_OPCODE, X64_INT3_BYTE,
    X64_JMP_MEM_REL32_OPCODES, X64_NOP_BYTE,
};
#[cfg(all(windows, feature = "private-uri"))]
use crate::structs::patch::AddLobbyAttributePrivateUriSpec;
use crate::structs::patch::{
    BytePatch, OwnerResetTrampolineSpec, PatchOutcome, PatchReport, PostUpdateUriResetSpec,
    SetUriStartTimeTrampolineSpec, TrampolineError,
};

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::HMODULE;
#[cfg(windows)]
use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
#[cfg(windows)]
use windows::Win32::System::Memory::{
    VirtualAlloc, VirtualProtect, VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_FREE,
    MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
};
#[cfg(windows)]
use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
#[cfg(windows)]
use windows::Win32::System::Threading::GetCurrentProcess;

/// Checks whether a full executable path or filename matches `DreadHunger-Win64-Shipping.exe`
/// case-insensitively.
#[must_use]
pub fn is_target_process_path(path: &str) -> bool {
    let trimmed: &str = path.trim_end_matches('\0');
    let basename: &str = trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed);
    basename.eq_ignore_ascii_case(TARGET_PROCESS_NAME)
}

/// Parses `SizeOfImage` from an in-memory PE32+ image header slice.
#[must_use]
pub fn parse_pe_size_of_image(header_bytes: &[u8]) -> Option<usize> {
    if header_bytes.len() < PE_MIN_DOS_BYTES {
        return None;
    }
    let dos_magic: u16 = u16::from_le_bytes([header_bytes[0], header_bytes[1]]);
    if dos_magic != PE_DOS_MAGIC {
        return None;
    }

    let lfanew_bytes: [u8; 4] = [
        header_bytes[PE_LFANEW_OFFSET],
        header_bytes[PE_LFANEW_OFFSET + 1],
        header_bytes[PE_LFANEW_OFFSET + 2],
        header_bytes[PE_LFANEW_OFFSET + 3],
    ];
    let lfanew: usize = usize::try_from(u32::from_le_bytes(lfanew_bytes)).ok()?;
    let nt_end: usize = lfanew.checked_add(PE_MIN_NT_BYTES)?;
    if header_bytes.len() < nt_end {
        return None;
    }

    let nt_sig: u32 = u32::from_le_bytes([
        header_bytes[lfanew],
        header_bytes[lfanew + 1],
        header_bytes[lfanew + 2],
        header_bytes[lfanew + 3],
    ]);
    if nt_sig != PE_NT_SIGNATURE {
        return None;
    }

    let opt_base: usize = lfanew.checked_add(PE_OPTIONAL_HEADER_OFFSET)?;
    let opt_magic: u16 = u16::from_le_bytes([header_bytes[opt_base], header_bytes[opt_base + 1]]);
    if opt_magic != PE_OPT_MAGIC_PE32_PLUS {
        return None;
    }

    let size_off: usize = opt_base.checked_add(PE_SIZE_OF_IMAGE_OFFSET)?;
    let size_of_image: u32 = u32::from_le_bytes([
        header_bytes[size_off],
        header_bytes[size_off + 1],
        header_bytes[size_off + 2],
        header_bytes[size_off + 3],
    ]);
    usize::try_from(size_of_image).ok()
}

/// Computes an x86-64 `call rel32` instruction (`E8 <rel32>`) from `call_addr` to `target_addr`.
pub fn encode_call_rel32(
    call_addr: usize,
    target_addr: usize,
) -> Result<[u8; X64_CALL_REL32_LEN], TrampolineError> {
    let next_rip: usize = call_addr
        .checked_add(X64_CALL_REL32_LEN)
        .ok_or(TrampolineError::AddressOverflow)?;
    let delta: i128 = i128::try_from(target_addr)
        .map_err(|_err: std::num::TryFromIntError| TrampolineError::AddressOverflow)?
        - i128::try_from(next_rip)
            .map_err(|_err: std::num::TryFromIntError| TrampolineError::AddressOverflow)?;
    let rel32: i32 = i32::try_from(delta)
        .map_err(|_err: std::num::TryFromIntError| TrampolineError::DisplacementOutOfRange)?;
    let rel_bytes: [u8; 4] = rel32.to_le_bytes();
    Ok([
        X64_CALL_REL32_OPCODE,
        rel_bytes[0],
        rel_bytes[1],
        rel_bytes[2],
        rel_bytes[3],
    ])
}

/// Computes an x86-64 RIP-relative indirect tail jump (`FF 25 <rel32>`,
/// `jmp qword ptr [rip + rel32]`) from `jmp_addr` through `iat_slot_addr`.
pub fn encode_jmp_indirect_rel32(
    jmp_addr: usize,
    iat_slot_addr: usize,
) -> Result<[u8; X64_CALL_MEM_REL32_LEN], TrampolineError> {
    let next_rip: usize = jmp_addr
        .checked_add(X64_CALL_MEM_REL32_LEN)
        .ok_or(TrampolineError::AddressOverflow)?;
    let delta: i128 = i128::try_from(iat_slot_addr)
        .map_err(|_err: std::num::TryFromIntError| TrampolineError::AddressOverflow)?
        - i128::try_from(next_rip)
            .map_err(|_err: std::num::TryFromIntError| TrampolineError::AddressOverflow)?;
    let rel32: i32 = i32::try_from(delta)
        .map_err(|_err: std::num::TryFromIntError| TrampolineError::DisplacementOutOfRange)?;
    let rel_bytes: [u8; 4] = rel32.to_le_bytes();
    Ok([
        X64_JMP_MEM_REL32_OPCODES[0],
        X64_JMP_MEM_REL32_OPCODES[1],
        rel_bytes[0],
        rel_bytes[1],
        rel_bytes[2],
        rel_bytes[3],
    ])
}

/// Encodes the 61-byte `RX` trampoline executed from `UDH_OnlineSession::PostUpdate` (`0xE8894B`):
///
/// Calls `FOnlineSessionSettings::Get<FString>` for `SessionSettingURIName`; if the attribute is
/// absent (`al == 0`) or empty (`SessionURI.ArrayNum <= 1`), calls `UDH_GameInstance::ResetURI`
/// on `UDH_GameInstance::GetInstance(rsi)` while preserving the exact boolean return value `al`.
/// When a non-empty server URI is present (during initial join or active play), leaves `+0x570`
/// untouched so deduplication prevents reconnect loops.
pub fn encode_post_update_empty_uri_trampoline(
    trampoline_addr: usize,
    original_get_addr: usize,
    get_game_instance_addr: usize,
    reset_uri_addr: usize,
) -> Result<[u8; POST_UPDATE_TRAMPOLINE_LEN], TrampolineError> {
    let call_get_addr: usize = trampoline_addr
        .checked_add(0x06)
        .ok_or(TrampolineError::AddressOverflow)?;
    let call_get: [u8; X64_CALL_REL32_LEN] = encode_call_rel32(call_get_addr, original_get_addr)?;

    let call_gi_addr: usize = trampoline_addr
        .checked_add(0x21)
        .ok_or(TrampolineError::AddressOverflow)?;
    let call_gi: [u8; X64_CALL_REL32_LEN] =
        encode_call_rel32(call_gi_addr, get_game_instance_addr)?;

    let call_reset_addr: usize = trampoline_addr
        .checked_add(0x2E)
        .ok_or(TrampolineError::AddressOverflow)?;
    let call_reset: [u8; X64_CALL_REL32_LEN] = encode_call_rel32(call_reset_addr, reset_uri_addr)?;

    let mut stub: [u8; POST_UPDATE_TRAMPOLINE_LEN] = [X64_INT3_BYTE; POST_UPDATE_TRAMPOLINE_LEN];
    stub[0x00..0x06].copy_from_slice(&POST_UPDATE_PROLOGUE);
    stub[0x06..0x0B].copy_from_slice(&call_get);
    stub[0x0B..0x1A].copy_from_slice(&POST_UPDATE_CHECK_EMPTY);
    stub[0x1A..0x21].copy_from_slice(&POST_UPDATE_PREP_GET_GI);
    stub[0x21..0x26].copy_from_slice(&call_gi);
    stub[0x26..0x2E].copy_from_slice(&TRAMPOLINE_CHECK_GI_NONNULL);
    stub[0x2E..0x33].copy_from_slice(&call_reset);
    stub[0x33..0x3D].copy_from_slice(&POST_UPDATE_EPILOGUE);
    Ok(stub)
}

/// Encodes the 41-byte `RX` trampoline executed from `UDH_OnlineSession::OnMatchEnded` (`0xE81B59`)
/// and `UDH_OnlineSession::ResetCurrentData` (`0xE8B3A3`):
///
/// Calls `UDH_OnlineSession::IsSessionOwner()`; when returning `true` (host resetting the lobby),
/// calls `UDH_GameInstance::ResetURI` on `UDH_GameInstance::GetInstance(rdi)` and returns `true`.
/// For non-host clients (`false`), returns `false` without prematurely clearing `+0x570` before
/// the host's EOS update clears `SessionSettingURIName`.
pub fn encode_owner_reset_trampoline(
    trampoline_addr: usize,
    is_session_owner_addr: usize,
    get_game_instance_addr: usize,
    reset_uri_addr: usize,
) -> Result<[u8; OWNER_RESET_TRAMPOLINE_LEN], TrampolineError> {
    let call_owner_addr: usize = trampoline_addr
        .checked_add(0x04)
        .ok_or(TrampolineError::AddressOverflow)?;
    let call_owner: [u8; X64_CALL_REL32_LEN] =
        encode_call_rel32(call_owner_addr, is_session_owner_addr)?;

    let call_gi_addr: usize = trampoline_addr
        .checked_add(0x10)
        .ok_or(TrampolineError::AddressOverflow)?;
    let call_gi: [u8; X64_CALL_REL32_LEN] =
        encode_call_rel32(call_gi_addr, get_game_instance_addr)?;

    let call_reset_addr: usize = trampoline_addr
        .checked_add(0x1D)
        .ok_or(TrampolineError::AddressOverflow)?;
    let call_reset: [u8; X64_CALL_REL32_LEN] = encode_call_rel32(call_reset_addr, reset_uri_addr)?;

    let mut stub: [u8; OWNER_RESET_TRAMPOLINE_LEN] = [X64_INT3_BYTE; OWNER_RESET_TRAMPOLINE_LEN];
    stub[0x00..0x04].copy_from_slice(&OWNER_RESET_PROLOGUE);
    stub[0x04..0x09].copy_from_slice(&call_owner);
    stub[0x09..0x10].copy_from_slice(&OWNER_RESET_CHECK_OWNER);
    stub[0x10..0x15].copy_from_slice(&call_gi);
    stub[0x15..0x1D].copy_from_slice(&TRAMPOLINE_CHECK_GI_NONNULL);
    stub[0x1D..0x22].copy_from_slice(&call_reset);
    stub[0x22..0x29].copy_from_slice(&OWNER_RESET_EPILOGUE);
    Ok(stub)
}

/// Encodes the 91-byte `RX` trampoline executed from `UDH_OnlineSession::SetURI` (`0xE7E386`):
///
/// Calls `FDateTime::UtcNow` (`0x1193830`), converts `FDateTime::Ticks` to Unix UTC seconds
/// `(ticks - 621_355_968_000_000_000) / 10_000_000` (clamped `>= 1` so `STARTUTCDATETIME` is
/// always strictly positive even on skewed system clocks), stores the value into
/// `UDH_OnlineSession::StartUTCDateTime` (`[this + 0x48]`), and invokes
/// `UDH_OnlineSession::Update(this)` (`0xE95760`) so `URI` and `STARTUTCDATETIME` are published
/// atomically in one EOS `UpdateLobby` call.
pub fn encode_set_uri_start_time_trampoline(
    trampoline_addr: usize,
    utc_now_addr: usize,
    update_session_addr: usize,
) -> Result<[u8; SET_URI_START_TIME_TRAMPOLINE_LEN], TrampolineError> {
    let call_utc_now_addr: usize = trampoline_addr
        .checked_add(0x17)
        .ok_or(TrampolineError::AddressOverflow)?;
    let call_utc_now: [u8; X64_CALL_REL32_LEN] =
        encode_call_rel32(call_utc_now_addr, utc_now_addr)?;

    let call_update_addr: usize = trampoline_addr
        .checked_add(0x51)
        .ok_or(TrampolineError::AddressOverflow)?;
    let call_update: [u8; X64_CALL_REL32_LEN] =
        encode_call_rel32(call_update_addr, update_session_addr)?;

    let mut stub: [u8; SET_URI_START_TIME_TRAMPOLINE_LEN] =
        [X64_INT3_BYTE; SET_URI_START_TIME_TRAMPOLINE_LEN];
    stub[0x00..0x17].copy_from_slice(&SET_URI_START_TIME_PROLOGUE);
    stub[0x17..0x1C].copy_from_slice(&call_utc_now);
    stub[0x1C..0x51].copy_from_slice(&SET_URI_START_TIME_CONVERT_AND_STORE);
    stub[0x51..0x56].copy_from_slice(&call_update);
    stub[0x56..0x5B].copy_from_slice(&SET_URI_START_TIME_EPILOGUE);
    Ok(stub)
}

/// Encodes the 75-byte `RX` trampoline executed from `FOnlineSessionEOS::AddLobbyAttribute`
/// (`0x9CA894`) in Plan B (`feature = "private-uri"`):
///
/// Checks `Options->Attribute->Key` byte-by-byte against ASCII `"URI"` (case-insensitive, exact
/// 3-char match terminated by `'\0'`). On match, sets `Options->Visibility = 1`
/// (`EOS_LAT_PRIVATE`) at `[rdx + 0x10]` so non-members querying EOS cannot see the dedicated
/// server IP before joining the lobby. All other keys keep `0` (`EOS_LAT_PUBLIC`). Preserves
/// `rcx`, `rdx`, stack alignment, and all callee-saved registers, then tail-jumps to the original
/// `EOS_LobbyModification_AddAttribute` IAT slot (`FF 25 <rel32>`).
pub fn encode_add_lobby_attribute_private_uri_trampoline(
    trampoline_addr: usize,
    iat_slot_addr: usize,
) -> Result<[u8; ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN], TrampolineError> {
    let jmp_addr: usize = trampoline_addr
        .checked_add(0x45)
        .ok_or(TrampolineError::AddressOverflow)?;
    let tail_jmp: [u8; X64_CALL_MEM_REL32_LEN] =
        encode_jmp_indirect_rel32(jmp_addr, iat_slot_addr)?;

    let mut stub: [u8; ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN] =
        [X64_INT3_BYTE; ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN];
    stub[0x00..0x45].copy_from_slice(&ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_CHECK);
    stub[0x45..0x4B].copy_from_slice(&tail_jmp);
    Ok(stub)
}

/// Aligns `value` upward to the next multiple of `alignment` (power of two or positive integer).
#[must_use]
pub fn align_up(value: usize, alignment: usize) -> Option<usize> {
    if alignment == 0 {
        return None;
    }
    let rem: usize = value % alignment;
    if rem == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - rem)
    }
}

/// Aligns `value` downward to a multiple of `alignment`.
#[must_use]
pub fn align_down(value: usize, alignment: usize) -> Option<usize> {
    if alignment == 0 {
        return None;
    }
    Some(value - (value % alignment))
}

/// Verifies and applies a single direct byte patch against a mutable memory buffer.
///
/// This pure helper is used by tests to validate exact byte-matching and idempotency logic.
#[must_use]
pub fn apply_direct_patch_in_buffer(buffer: &mut [u8], patch: &BytePatch) -> PatchOutcome {
    if patch.expected.len() != patch.replacement.len() {
        return PatchOutcome::SignatureMismatch;
    }
    let Some(end): Option<usize> = patch.rva.checked_add(patch.expected.len()) else {
        return PatchOutcome::OutOfBounds;
    };
    let Some(slice): Option<&mut [u8]> = buffer.get_mut(patch.rva..end) else {
        return PatchOutcome::OutOfBounds;
    };
    if slice == patch.replacement {
        return PatchOutcome::AlreadyApplied;
    }
    if slice != patch.expected {
        return PatchOutcome::SignatureMismatch;
    }
    slice.copy_from_slice(patch.replacement);
    PatchOutcome::Applied
}

/// Applies all patches to the current process if and only if it is `DreadHunger-Win64-Shipping.exe`.
#[cfg(windows)]
#[must_use]
pub fn apply_client_patches_if_target() -> PatchReport {
    let Some(exe_path): Option<String> = current_process_exe_path() else {
        return PatchReport {
            target_matched: false,
            applied: 0,
            already_applied: 0,
            skipped: 0,
        };
    };

    if !is_target_process_path(&exe_path) {
        return PatchReport {
            target_matched: false,
            applied: 0,
            already_applied: 0,
            skipped: 0,
        };
    }

    // SAFETY: Passing `None` to `GetModuleHandleW` returns the already-loaded base HMODULE
    // of the current process image without acquiring or mutating loader locks.
    let Ok(module_handle): Result<HMODULE, windows::core::Error> =
        (unsafe { GetModuleHandleW(PCWSTR::null()) })
    else {
        return PatchReport {
            target_matched: true,
            applied: 0,
            already_applied: 0,
            skipped: TOTAL_PATCH_COUNT,
        };
    };

    let module_base: usize = module_handle.0 as usize;
    let Some(image_size): Option<usize> = read_live_image_size(module_base) else {
        return PatchReport {
            target_matched: true,
            applied: 0,
            already_applied: 0,
            skipped: TOTAL_PATCH_COUNT,
        };
    };

    let mut report: PatchReport = PatchReport {
        target_matched: true,
        applied: 0,
        already_applied: 0,
        skipped: 0,
    };

    for patch in &DIRECT_PATCHES {
        let outcome: PatchOutcome = apply_live_direct_patch(module_base, image_size, patch);
        record_outcome(&mut report, outcome);
    }

    let post_update_outcome: PatchOutcome =
        apply_live_post_update_trampoline(module_base, image_size, &POST_UPDATE_EMPTY_URI_RESET);
    record_outcome(&mut report, post_update_outcome);

    for owner_spec in &OWNER_RESET_TRAMPOLINES {
        let owner_outcome: PatchOutcome =
            apply_live_owner_reset_trampoline(module_base, image_size, owner_spec);
        record_outcome(&mut report, owner_outcome);
    }

    let set_uri_start_time_outcome: PatchOutcome = apply_live_set_uri_start_time_trampoline(
        module_base,
        image_size,
        &SET_URI_START_TIME_TRAMPOLINE,
    );
    record_outcome(&mut report, set_uri_start_time_outcome);

    #[cfg(feature = "private-uri")]
    {
        let private_uri_outcome: PatchOutcome =
            apply_live_add_lobby_attribute_private_uri_trampoline(
                module_base,
                image_size,
                &ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE,
            );
        record_outcome(&mut report, private_uri_outcome);
    }

    report
}

/// Non-Windows stub returning an empty report.
#[cfg(not(windows))]
#[must_use]
pub fn apply_client_patches_if_target() -> PatchReport {
    PatchReport {
        target_matched: false,
        applied: 0,
        already_applied: 0,
        skipped: 0,
    }
}

#[cfg(windows)]
fn record_outcome(report: &mut PatchReport, outcome: PatchOutcome) {
    match outcome {
        PatchOutcome::Applied => {
            report.applied += 1;
        }
        PatchOutcome::AlreadyApplied => {
            report.already_applied += 1;
        }
        PatchOutcome::OutOfBounds | PatchOutcome::SignatureMismatch | PatchOutcome::MemoryError => {
            report.skipped += 1;
        }
    }
}

#[cfg(windows)]
fn current_process_exe_path() -> Option<String> {
    let mut capacity: usize = crate::consts::proxy::WIN32_PATH_BUF_CAPACITY;
    while capacity <= crate::consts::proxy::WIN32_PATH_BUF_MAX {
        let mut buf: Vec<u16> = vec![0u16; capacity];
        // SAFETY: `buf` is a writable UTF-16 buffer of length `capacity`, and passing `None`
        // queries the path of the current process image.
        let written: u32 = unsafe { GetModuleFileNameW(None, &mut buf) };
        if written == 0 {
            return None;
        }
        let written_usize: usize = usize::try_from(written).ok()?;
        if written_usize < capacity {
            return Some(String::from_utf16_lossy(&buf[..written_usize]));
        }
        capacity = capacity.checked_mul(2)?;
    }
    None
}

#[cfg(windows)]
fn read_live_image_size(module_base: usize) -> Option<usize> {
    if module_base == 0 {
        return None;
    }
    // SAFETY: The first page (`0x1000` bytes) of the main executable image is always mapped
    // and readable while the process is running.
    let header_slice: &[u8] =
        unsafe { std::slice::from_raw_parts(module_base as *const u8, TRAMPOLINE_ALLOC_SIZE) };
    parse_pe_size_of_image(header_slice)
}

#[cfg(windows)]
fn apply_live_direct_patch(
    module_base: usize,
    image_size: usize,
    patch: &BytePatch,
) -> PatchOutcome {
    if patch.expected.len() != patch.replacement.len() {
        return PatchOutcome::SignatureMismatch;
    }
    let Some(end_rva): Option<usize> = patch.rva.checked_add(patch.expected.len()) else {
        return PatchOutcome::OutOfBounds;
    };
    if end_rva > image_size {
        return PatchOutcome::OutOfBounds;
    }
    let Some(target_addr): Option<usize> = module_base.checked_add(patch.rva) else {
        return PatchOutcome::OutOfBounds;
    };

    // SAFETY: `target_addr..target_addr + expected.len()` is bounds-checked within the mapped
    // process image (`[module_base, module_base + image_size)`).
    let current_bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(target_addr as *const u8, patch.expected.len()) };
    if current_bytes == patch.replacement {
        return PatchOutcome::AlreadyApplied;
    }
    if current_bytes != patch.expected {
        return PatchOutcome::SignatureMismatch;
    }

    write_protected_code(target_addr, patch.replacement)
}

#[cfg(windows)]
fn apply_live_post_update_trampoline(
    module_base: usize,
    image_size: usize,
    spec: &PostUpdateUriResetSpec,
) -> PatchOutcome {
    let Some(call_end_rva): Option<usize> = spec.call_site_rva.checked_add(X64_CALL_REL32_LEN)
    else {
        return PatchOutcome::OutOfBounds;
    };
    if call_end_rva > image_size
        || spec.original_get_rva >= image_size
        || spec.get_game_instance_rva >= image_size
        || spec.reset_uri_rva >= image_size
    {
        return PatchOutcome::OutOfBounds;
    }

    let Some(call_site_addr): Option<usize> = module_base.checked_add(spec.call_site_rva) else {
        return PatchOutcome::OutOfBounds;
    };
    let Some(original_get_addr): Option<usize> = module_base.checked_add(spec.original_get_rva)
    else {
        return PatchOutcome::OutOfBounds;
    };
    let Some(get_gi_addr): Option<usize> = module_base.checked_add(spec.get_game_instance_rva)
    else {
        return PatchOutcome::OutOfBounds;
    };
    let Some(reset_uri_addr): Option<usize> = module_base.checked_add(spec.reset_uri_rva) else {
        return PatchOutcome::OutOfBounds;
    };

    // SAFETY: `call_site_addr..call_site_addr + 5` is bounds-checked inside the mapped image.
    let current_call: &[u8] =
        unsafe { std::slice::from_raw_parts(call_site_addr as *const u8, X64_CALL_REL32_LEN) };
    if current_call != spec.expected_call_bytes {
        return PatchOutcome::SignatureMismatch;
    }

    let Some(trampoline_addr): Option<usize> =
        allocate_nearby_rw_page(module_base, image_size, TRAMPOLINE_ALLOC_SIZE)
    else {
        return PatchOutcome::MemoryError;
    };

    let Ok(stub_bytes): Result<[u8; POST_UPDATE_TRAMPOLINE_LEN], TrampolineError> =
        encode_post_update_empty_uri_trampoline(
            trampoline_addr,
            original_get_addr,
            get_gi_addr,
            reset_uri_addr,
        )
    else {
        return PatchOutcome::MemoryError;
    };

    install_rx_trampoline_and_patch_call(call_site_addr, trampoline_addr, &stub_bytes)
}

#[cfg(windows)]
fn apply_live_owner_reset_trampoline(
    module_base: usize,
    image_size: usize,
    spec: &OwnerResetTrampolineSpec,
) -> PatchOutcome {
    let Some(call_end_rva): Option<usize> = spec.call_site_rva.checked_add(X64_CALL_REL32_LEN)
    else {
        return PatchOutcome::OutOfBounds;
    };
    if call_end_rva > image_size
        || spec.is_session_owner_rva >= image_size
        || spec.get_game_instance_rva >= image_size
        || spec.reset_uri_rva >= image_size
    {
        return PatchOutcome::OutOfBounds;
    }

    let Some(call_site_addr): Option<usize> = module_base.checked_add(spec.call_site_rva) else {
        return PatchOutcome::OutOfBounds;
    };
    let Some(is_owner_addr): Option<usize> = module_base.checked_add(spec.is_session_owner_rva)
    else {
        return PatchOutcome::OutOfBounds;
    };
    let Some(get_gi_addr): Option<usize> = module_base.checked_add(spec.get_game_instance_rva)
    else {
        return PatchOutcome::OutOfBounds;
    };
    let Some(reset_uri_addr): Option<usize> = module_base.checked_add(spec.reset_uri_rva) else {
        return PatchOutcome::OutOfBounds;
    };

    // SAFETY: `call_site_addr..call_site_addr + 5` is bounds-checked inside the mapped image.
    let current_call: &[u8] =
        unsafe { std::slice::from_raw_parts(call_site_addr as *const u8, X64_CALL_REL32_LEN) };
    if current_call != spec.expected_call_bytes {
        return PatchOutcome::SignatureMismatch;
    }

    let Some(trampoline_addr): Option<usize> =
        allocate_nearby_rw_page(module_base, image_size, TRAMPOLINE_ALLOC_SIZE)
    else {
        return PatchOutcome::MemoryError;
    };

    let Ok(stub_bytes): Result<[u8; OWNER_RESET_TRAMPOLINE_LEN], TrampolineError> =
        encode_owner_reset_trampoline(trampoline_addr, is_owner_addr, get_gi_addr, reset_uri_addr)
    else {
        return PatchOutcome::MemoryError;
    };

    install_rx_trampoline_and_patch_call(call_site_addr, trampoline_addr, &stub_bytes)
}

#[cfg(windows)]
fn apply_live_set_uri_start_time_trampoline(
    module_base: usize,
    image_size: usize,
    spec: &SetUriStartTimeTrampolineSpec,
) -> PatchOutcome {
    let Some(call_end_rva): Option<usize> = spec.call_site_rva.checked_add(X64_CALL_REL32_LEN)
    else {
        return PatchOutcome::OutOfBounds;
    };
    if call_end_rva > image_size
        || spec.utc_now_rva >= image_size
        || spec.update_session_rva >= image_size
    {
        return PatchOutcome::OutOfBounds;
    }

    let Some(call_site_addr): Option<usize> = module_base.checked_add(spec.call_site_rva) else {
        return PatchOutcome::OutOfBounds;
    };
    let Some(utc_now_addr): Option<usize> = module_base.checked_add(spec.utc_now_rva) else {
        return PatchOutcome::OutOfBounds;
    };
    let Some(update_session_addr): Option<usize> = module_base.checked_add(spec.update_session_rva)
    else {
        return PatchOutcome::OutOfBounds;
    };

    // SAFETY: `call_site_addr..call_site_addr + 5` is bounds-checked inside the mapped image.
    let current_call: &[u8] =
        unsafe { std::slice::from_raw_parts(call_site_addr as *const u8, X64_CALL_REL32_LEN) };
    if current_call != spec.expected_call_bytes {
        return PatchOutcome::SignatureMismatch;
    }

    let Some(trampoline_addr): Option<usize> =
        allocate_nearby_rw_page(module_base, image_size, TRAMPOLINE_ALLOC_SIZE)
    else {
        return PatchOutcome::MemoryError;
    };

    let Ok(stub_bytes): Result<[u8; SET_URI_START_TIME_TRAMPOLINE_LEN], TrampolineError> =
        encode_set_uri_start_time_trampoline(trampoline_addr, utc_now_addr, update_session_addr)
    else {
        return PatchOutcome::MemoryError;
    };

    install_rx_trampoline_and_patch_call(call_site_addr, trampoline_addr, &stub_bytes)
}

#[cfg(all(windows, feature = "private-uri"))]
fn apply_live_add_lobby_attribute_private_uri_trampoline(
    module_base: usize,
    image_size: usize,
    spec: &AddLobbyAttributePrivateUriSpec,
) -> PatchOutcome {
    let Some(call_end_rva): Option<usize> = spec.call_site_rva.checked_add(X64_CALL_MEM_REL32_LEN)
    else {
        return PatchOutcome::OutOfBounds;
    };
    let Some(iat_end_rva): Option<usize> =
        spec.iat_slot_rva.checked_add(std::mem::size_of::<usize>())
    else {
        return PatchOutcome::OutOfBounds;
    };
    if call_end_rva > image_size || iat_end_rva > image_size {
        return PatchOutcome::OutOfBounds;
    }

    let Some(call_site_addr): Option<usize> = module_base.checked_add(spec.call_site_rva) else {
        return PatchOutcome::OutOfBounds;
    };
    let Some(iat_slot_addr): Option<usize> = module_base.checked_add(spec.iat_slot_rva) else {
        return PatchOutcome::OutOfBounds;
    };

    // SAFETY: `call_site_addr..call_site_addr + 6` is bounds-checked inside the mapped image.
    let current_call: &[u8] =
        unsafe { std::slice::from_raw_parts(call_site_addr as *const u8, X64_CALL_MEM_REL32_LEN) };
    if current_call != spec.expected_call_bytes {
        return PatchOutcome::SignatureMismatch;
    }

    let Some(trampoline_addr): Option<usize> =
        allocate_nearby_rw_page(module_base, image_size, TRAMPOLINE_ALLOC_SIZE)
    else {
        return PatchOutcome::MemoryError;
    };

    let Ok(stub_bytes): Result<
        [u8; ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN],
        TrampolineError,
    > = encode_add_lobby_attribute_private_uri_trampoline(trampoline_addr, iat_slot_addr) else {
        return PatchOutcome::MemoryError;
    };

    install_rx_trampoline_and_patch_site(call_site_addr, trampoline_addr, &stub_bytes, true)
}

#[cfg(windows)]
fn install_rx_trampoline_and_patch_call(
    call_site_addr: usize,
    trampoline_addr: usize,
    stub_bytes: &[u8],
) -> PatchOutcome {
    install_rx_trampoline_and_patch_site(call_site_addr, trampoline_addr, stub_bytes, false)
}

#[cfg(windows)]
fn install_rx_trampoline_and_patch_site(
    call_site_addr: usize,
    trampoline_addr: usize,
    stub_bytes: &[u8],
    pad_trailing_nop: bool,
) -> PatchOutcome {
    if stub_bytes.len() > TRAMPOLINE_ALLOC_SIZE {
        return PatchOutcome::MemoryError;
    }

    let Ok(patched_call): Result<[u8; X64_CALL_REL32_LEN], TrampolineError> =
        encode_call_rel32(call_site_addr, trampoline_addr)
    else {
        return PatchOutcome::MemoryError;
    };

    // SAFETY: `trampoline_addr` was freshly committed as `PAGE_READWRITE` with size
    // `TRAMPOLINE_ALLOC_SIZE` (`>= stub_bytes.len()`).
    unsafe {
        let page_slice: &mut [u8] =
            std::slice::from_raw_parts_mut(trampoline_addr as *mut u8, TRAMPOLINE_ALLOC_SIZE);
        page_slice.fill(X64_INT3_BYTE);
        page_slice[..stub_bytes.len()].copy_from_slice(stub_bytes);
    }

    let mut old_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Switching our newly written trampoline page from `RW` to `RX` (no persistent `RWX`)
    // before redirecting execution to it.
    let protect_result: Result<(), windows::core::Error> = unsafe {
        VirtualProtect(
            trampoline_addr as *const c_void,
            TRAMPOLINE_ALLOC_SIZE,
            PAGE_EXECUTE_READ,
            &raw mut old_protect,
        )
    };
    if protect_result.is_err() {
        return PatchOutcome::MemoryError;
    }

    // SAFETY: Flush CPU instruction cache for the freshly written executable trampoline page.
    let _flushed: Result<(), windows::core::Error> = unsafe {
        FlushInstructionCache(
            GetCurrentProcess(),
            Some(trampoline_addr as *const c_void),
            TRAMPOLINE_ALLOC_SIZE,
        )
    };

    if pad_trailing_nop {
        let patched_site: [u8; X64_CALL_MEM_REL32_LEN] = [
            patched_call[0],
            patched_call[1],
            patched_call[2],
            patched_call[3],
            patched_call[4],
            X64_NOP_BYTE,
        ];
        write_protected_code(call_site_addr, &patched_site)
    } else {
        write_protected_code(call_site_addr, &patched_call)
    }
}

#[cfg(windows)]
fn allocate_nearby_rw_page(module_base: usize, image_size: usize, size: usize) -> Option<usize> {
    let mut sys_info: SYSTEM_INFO = SYSTEM_INFO::default();
    // SAFETY: `GetSystemInfo` populates the provided `SYSTEM_INFO` struct.
    unsafe {
        GetSystemInfo(&raw mut sys_info);
    }
    let granularity: usize = usize::try_from(sys_info.dwAllocationGranularity)
        .ok()
        .filter(|value: &usize| *value > 0)
        .unwrap_or(DEFAULT_ALLOC_GRANULARITY);
    let min_app_addr: usize = sys_info.lpMinimumApplicationAddress as usize;
    let max_app_addr: usize = sys_info.lpMaximumApplicationAddress as usize;

    let search_after_start: usize = align_up(module_base.checked_add(image_size)?, granularity)?;
    let search_after_limit: usize = module_base
        .saturating_add(TRAMPOLINE_MAX_DISTANCE)
        .min(max_app_addr);

    if let Some(addr) =
        search_free_slot_ascending(search_after_start, search_after_limit, granularity, size)
    {
        return Some(addr);
    }

    let search_before_floor: usize = module_base
        .saturating_sub(TRAMPOLINE_MAX_DISTANCE)
        .max(min_app_addr);
    let search_before_start: usize = align_down(module_base, granularity)?;
    search_free_slot_descending(search_before_start, search_before_floor, granularity, size)
}

#[cfg(windows)]
fn search_free_slot_ascending(
    start: usize,
    limit: usize,
    granularity: usize,
    size: usize,
) -> Option<usize> {
    let mut cursor: usize = start;
    while cursor < limit {
        let mut mbi: MEMORY_BASIC_INFORMATION = MEMORY_BASIC_INFORMATION::default();
        // SAFETY: Querying virtual address space layout with `VirtualQuery` is read-only.
        let queried: usize = unsafe {
            VirtualQuery(
                Some(cursor as *const c_void),
                &raw mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if queried == 0 {
            break;
        }
        let region_base: usize = mbi.BaseAddress as usize;
        let region_end: usize = region_base.saturating_add(mbi.RegionSize);
        if mbi.State == MEM_FREE {
            if let Some(candidate) = align_up(cursor.max(region_base), granularity) {
                if candidate.saturating_add(size) <= region_end && candidate < limit {
                    if let Some(allocated) = try_commit_rw(candidate, size) {
                        return Some(allocated);
                    }
                }
            }
        }
        if region_end <= cursor {
            break;
        }
        cursor = region_end;
    }
    None
}

#[cfg(windows)]
fn search_free_slot_descending(
    start: usize,
    floor: usize,
    granularity: usize,
    size: usize,
) -> Option<usize> {
    let mut cursor: usize = start;
    while cursor > floor {
        let Some(probe): Option<usize> = cursor.checked_sub(granularity) else {
            break;
        };
        if probe < floor {
            break;
        }
        let mut mbi: MEMORY_BASIC_INFORMATION = MEMORY_BASIC_INFORMATION::default();
        // SAFETY: Querying virtual address space layout with `VirtualQuery` is read-only.
        let queried: usize = unsafe {
            VirtualQuery(
                Some(probe as *const c_void),
                &raw mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if queried == 0 {
            break;
        }
        let region_base: usize = mbi.BaseAddress as usize;
        let region_end: usize = region_base.saturating_add(mbi.RegionSize);
        if mbi.State == MEM_FREE {
            if let Some(candidate) = align_down(probe, granularity) {
                if candidate >= region_base && candidate.saturating_add(size) <= region_end {
                    if let Some(allocated) = try_commit_rw(candidate, size) {
                        return Some(allocated);
                    }
                }
            }
        }
        if region_base >= cursor {
            break;
        }
        cursor = region_base;
    }
    None
}

#[cfg(windows)]
fn try_commit_rw(address: usize, size: usize) -> Option<usize> {
    // SAFETY: Requesting a fresh private `PAGE_READWRITE` page at a free virtual address.
    let ptr: *mut c_void = unsafe {
        VirtualAlloc(
            Some(address as *const c_void),
            size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    if ptr.is_null() {
        None
    } else {
        Some(ptr as usize)
    }
}

#[cfg(windows)]
fn write_protected_code(target_addr: usize, replacement: &[u8]) -> PatchOutcome {
    let mut old_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Temporarily marks the verified code address writable, writes exact replacement
    // bytes, restores the original protection immediately, and flushes the instruction cache.
    let protect_ok: Result<(), windows::core::Error> = unsafe {
        VirtualProtect(
            target_addr as *const c_void,
            replacement.len(),
            PAGE_EXECUTE_READWRITE,
            &raw mut old_protect,
        )
    };
    if protect_ok.is_err() {
        return PatchOutcome::MemoryError;
    }

    // SAFETY: `target_addr` is now writable for `replacement.len()` bytes inside the process image.
    unsafe {
        std::ptr::copy_nonoverlapping(
            replacement.as_ptr(),
            target_addr as *mut u8,
            replacement.len(),
        );
    }

    let mut ignored_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Restores the page's previous protection (`PAGE_EXECUTE_READ`) so no `RWX` remains.
    let _restore_ok: Result<(), windows::core::Error> = unsafe {
        VirtualProtect(
            target_addr as *const c_void,
            replacement.len(),
            old_protect,
            &raw mut ignored_protect,
        )
    };

    // SAFETY: Ensures CPU instruction prefetch buffers observe the patched machine code.
    let _flush_ok: Result<(), windows::core::Error> = unsafe {
        FlushInstructionCache(
            GetCurrentProcess(),
            Some(target_addr as *const c_void),
            replacement.len(),
        )
    };

    PatchOutcome::Applied
}
