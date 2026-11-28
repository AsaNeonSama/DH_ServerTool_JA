use crate::consts::patch::{
    EOS_RECV_GUARD_CHECK, EOS_RECV_GUARD_STUB_LEN, PE_DOS_MAGIC, PE_LFANEW_OFFSET,
    PE_MIN_DOS_BYTES, PE_MIN_NT_BYTES, PE_NT_SIGNATURE, PE_OPTIONAL_HEADER_OFFSET,
    PE_OPT_MAGIC_PE32_PLUS, PE_SIZE_OF_IMAGE_OFFSET, TARGET_PROCESS_NAME, X64_ABS_JMP_LEN,
    X64_INT3_BYTE, X64_JMP_ABS_RIP0_PREFIX, X64_JMP_REL32_LEN, X64_JMP_REL32_OPCODE,
    X64_MOV_RAX_IMM64_PREFIX,
};
use crate::structs::patch::{PatchReport, TrampolineError};

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

#[cfg(windows)]
use crate::consts::patch::{
    DEFAULT_ALLOC_GRANULARITY, DETOUR_ENGINE_LOOP_TICK, DETOUR_GET_WANTS_TO_TALK,
    DETOUR_IS_PARTICIPANT_SPEAKING, DETOUR_IS_PLAYER_IN_VOICE_CHAT, DETOUR_SET_WANTS_TO_TALK,
    DETOUR_SOCKET_CLOSE, DETOUR_SOCKET_RECV_FROM, DETOUR_SOCKET_SEND_TO,
    EOS_BEFORE_RECV_AUDIO_GUARD, STANDARD_DETOUR_COUNT, TOTAL_PATCH_COUNT, TRAMPOLINE_ALLOC_SIZE,
    TRAMPOLINE_MAX_DISTANCE, TRAMPOLINE_SLOT_ALIGN, X64_NOP_BYTE,
};
#[cfg(windows)]
use crate::hooks::{
    detour_engine_loop_tick, detour_get_wants_to_talk, detour_is_participant_speaking,
    detour_is_player_in_voice_chat, detour_set_wants_to_talk, detour_socket_close,
    detour_socket_recv_from, detour_socket_send_to, set_module_image, set_orig_engine_loop_tick,
    set_orig_get_wants_to_talk, set_orig_is_participant_speaking, set_orig_is_player_in_voice_chat,
    set_orig_set_wants_to_talk, set_orig_socket_close, set_orig_socket_recv_from,
    set_orig_socket_send_to, IN_DEDICATED_MATCH,
};
#[cfg(windows)]
use crate::logger::log_line;
#[cfg(windows)]
use crate::structs::patch::{
    EosRecvGuardSpec, PatchOutcome, PendingSitePatch, StandardDetourBinding, TrampolinePageBuilder,
};

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

/// Computes an x86-64 `jmp rel32` instruction (`E9 <rel32>`) from `jmp_addr` to `target_addr`.
pub fn encode_jmp_rel32(
    jmp_addr: usize,
    target_addr: usize,
) -> Result<[u8; X64_JMP_REL32_LEN], TrampolineError> {
    let next_rip: usize = jmp_addr
        .checked_add(X64_JMP_REL32_LEN)
        .ok_or(TrampolineError::AddressOverflow)?;
    let delta: i128 = i128::try_from(target_addr)
        .map_err(|_err: std::num::TryFromIntError| TrampolineError::AddressOverflow)?
        - i128::try_from(next_rip)
            .map_err(|_err: std::num::TryFromIntError| TrampolineError::AddressOverflow)?;
    let rel32: i32 = i32::try_from(delta)
        .map_err(|_err: std::num::TryFromIntError| TrampolineError::DisplacementOutOfRange)?;
    let rel_bytes: [u8; 4] = rel32.to_le_bytes();
    Ok([
        X64_JMP_REL32_OPCODE,
        rel_bytes[0],
        rel_bytes[1],
        rel_bytes[2],
        rel_bytes[3],
    ])
}

/// Encodes a register-preserving 14-byte absolute jump (`jmp qword ptr [rip + 0]; dq target_addr`).
pub fn encode_abs_jmp_stub(target_addr: usize) -> Result<[u8; X64_ABS_JMP_LEN], TrampolineError> {
    let target_u64: u64 = u64::try_from(target_addr)
        .map_err(|_err: std::num::TryFromIntError| TrampolineError::AddressOverflow)?;
    let mut stub: [u8; X64_ABS_JMP_LEN] = [0u8; X64_ABS_JMP_LEN];
    stub[..6].copy_from_slice(&X64_JMP_ABS_RIP0_PREFIX);
    stub[6..].copy_from_slice(&target_u64.to_le_bytes());
    Ok(stub)
}

/// Encodes an original-call trampoline (`stolen_prologue` + 14-byte absolute jump to `resume_addr`).
pub fn encode_original_trampoline(
    stolen_prologue: &[u8],
    resume_addr: usize,
) -> Result<Vec<u8>, TrampolineError> {
    let abs_jmp: [u8; X64_ABS_JMP_LEN] = encode_abs_jmp_stub(resume_addr)?;
    let mut bytes: Vec<u8> = Vec::with_capacity(stolen_prologue.len() + X64_ABS_JMP_LEN);
    bytes.extend_from_slice(stolen_prologue);
    bytes.extend_from_slice(&abs_jmp);
    Ok(bytes)
}

/// Encodes the 40-byte zero-overhead guard stub for
/// `FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered` (`0xA2DC70`):
/// returns immediately when `*(flag_addr as *const u8) != 0`; otherwise executes the stolen
/// 10-byte prologue and jumps directly back to `resume_addr` (`0xA2DC7A`) without touching any
/// arguments or stack slots.
pub fn encode_eos_recv_guard_stub(
    flag_addr: usize,
    expected_prologue: &[u8; 10],
    resume_addr: usize,
) -> Result<[u8; EOS_RECV_GUARD_STUB_LEN], TrampolineError> {
    let flag_u64: u64 = u64::try_from(flag_addr)
        .map_err(|_err: std::num::TryFromIntError| TrampolineError::AddressOverflow)?;
    let abs_resume: [u8; X64_ABS_JMP_LEN] = encode_abs_jmp_stub(resume_addr)?;

    let mut stub: [u8; EOS_RECV_GUARD_STUB_LEN] = [X64_INT3_BYTE; EOS_RECV_GUARD_STUB_LEN];
    stub[0..2].copy_from_slice(&X64_MOV_RAX_IMM64_PREFIX);
    stub[2..10].copy_from_slice(&flag_u64.to_le_bytes());
    stub[10..16].copy_from_slice(&EOS_RECV_GUARD_CHECK);
    stub[16..26].copy_from_slice(expected_prologue);
    stub[26..40].copy_from_slice(&abs_resume);
    Ok(stub)
}

/// Aligns `value` upward to the next multiple of `alignment`.
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

/// Applies all 9 Plan A pure-Rust client voice detours if and only if running inside
/// `DreadHunger-Win64-Shipping.exe`.
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

    // SAFETY: Passing null queries the current process image handle without taking loader locks.
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

    set_module_image(module_base, image_size);

    let mut report: PatchReport = PatchReport {
        target_matched: true,
        applied: 0,
        already_applied: 0,
        skipped: 0,
    };

    install_all_client_trampolines(module_base, image_size, &mut report);

    log_line(&format!(
        "Plan A voice startup summary: applied={}, already_applied={}, skipped={}, image_base=0x{module_base:X}, image_size=0x{image_size:X}",
        report.applied, report.already_applied, report.skipped
    ));

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
fn install_all_client_trampolines(module_base: usize, image_size: usize, report: &mut PatchReport) {
    let Some(page_addr): Option<usize> =
        allocate_nearby_rw_page(module_base, image_size, TRAMPOLINE_ALLOC_SIZE)
    else {
        log_line(&format!(
            "failed to allocate nearby RX trampoline page; skipping {TOTAL_PATCH_COUNT} detours"
        ));
        report.skipped += TOTAL_PATCH_COUNT;
        return;
    };

    // SAFETY: `page_addr` is a freshly committed private `PAGE_READWRITE` page of `0x1000` bytes.
    let page_slice: &mut [u8] =
        unsafe { std::slice::from_raw_parts_mut(page_addr as *mut u8, TRAMPOLINE_ALLOC_SIZE) };
    page_slice.fill(X64_INT3_BYTE);

    let mut builder: TrampolinePageBuilder<'_> = TrampolinePageBuilder {
        module_base,
        image_size,
        page_addr,
        page_slice,
        cursor: 0,
        pending_patches: Vec::with_capacity(TOTAL_PATCH_COUNT),
    };

    // 1. Zero-overhead guard on `FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered`.
    prepare_eos_recv_guard(
        &mut builder,
        &EOS_BEFORE_RECV_AUDIO_GUARD,
        (&raw const IN_DEDICATED_MATCH) as usize,
        report,
    );

    // 2..9. Eight standard function-entry detours with original-call trampolines.
    let standard_detours: [StandardDetourBinding; STANDARD_DETOUR_COUNT] = [
        StandardDetourBinding {
            spec: &DETOUR_SOCKET_RECV_FROM,
            handler_addr: detour_socket_recv_from as *const () as usize,
            register_orig: set_orig_socket_recv_from,
        },
        StandardDetourBinding {
            spec: &DETOUR_SOCKET_SEND_TO,
            handler_addr: detour_socket_send_to as *const () as usize,
            register_orig: set_orig_socket_send_to,
        },
        StandardDetourBinding {
            spec: &DETOUR_SOCKET_CLOSE,
            handler_addr: detour_socket_close as *const () as usize,
            register_orig: set_orig_socket_close,
        },
        StandardDetourBinding {
            spec: &DETOUR_ENGINE_LOOP_TICK,
            handler_addr: detour_engine_loop_tick as *const () as usize,
            register_orig: set_orig_engine_loop_tick,
        },
        StandardDetourBinding {
            spec: &DETOUR_SET_WANTS_TO_TALK,
            handler_addr: detour_set_wants_to_talk as *const () as usize,
            register_orig: set_orig_set_wants_to_talk,
        },
        StandardDetourBinding {
            spec: &DETOUR_GET_WANTS_TO_TALK,
            handler_addr: detour_get_wants_to_talk as *const () as usize,
            register_orig: set_orig_get_wants_to_talk,
        },
        StandardDetourBinding {
            spec: &DETOUR_IS_PARTICIPANT_SPEAKING,
            handler_addr: detour_is_participant_speaking as *const () as usize,
            register_orig: set_orig_is_participant_speaking,
        },
        StandardDetourBinding {
            spec: &DETOUR_IS_PLAYER_IN_VOICE_CHAT,
            handler_addr: detour_is_player_in_voice_chat as *const () as usize,
            register_orig: set_orig_is_player_in_voice_chat,
        },
    ];

    for binding in standard_detours {
        prepare_standard_detour(&mut builder, binding, report);
    }

    let pending_patches: Vec<PendingSitePatch> = builder.pending_patches;

    // Transition the entire shared trampoline page strictly from `RW` to `RX` (zero persistent `RWX`)
    // BEFORE activating any `E9 rel32` site jumps.
    let mut old_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Switching our populated trampoline page to `PAGE_EXECUTE_READ`.
    let protect_res: Result<(), windows::core::Error> = unsafe {
        VirtualProtect(
            page_addr as *const c_void,
            TRAMPOLINE_ALLOC_SIZE,
            PAGE_EXECUTE_READ,
            &raw mut old_protect,
        )
    };
    if protect_res.is_err() {
        log_line("VirtualProtect(PAGE_EXECUTE_READ) failed on trampoline page");
        report.skipped += pending_patches.len();
        return;
    }

    // SAFETY: Flushing CPU instruction cache for the executable trampoline page.
    let _flushed: Result<(), windows::core::Error> = unsafe {
        FlushInstructionCache(
            GetCurrentProcess(),
            Some(page_addr as *const c_void),
            TRAMPOLINE_ALLOC_SIZE,
        )
    };

    for patch in pending_patches {
        let outcome: PatchOutcome = write_protected_code(patch.site_addr, &patch.patch_bytes);
        log_line(&format!("detour '{}': {outcome:?}", patch.name));
        record_outcome(report, outcome);
    }
}

#[cfg(windows)]
fn prepare_eos_recv_guard(
    builder: &mut TrampolinePageBuilder<'_>,
    spec: &EosRecvGuardSpec,
    flag_addr: usize,
    report: &mut PatchReport,
) {
    let stolen_len: usize = spec.expected_prologue.len();
    let Some(end_rva): Option<usize> = spec.rva.checked_add(stolen_len) else {
        record_outcome(report, PatchOutcome::OutOfBounds);
        return;
    };
    if end_rva > builder.image_size {
        record_outcome(report, PatchOutcome::OutOfBounds);
        return;
    }
    let Some(site_addr): Option<usize> = builder.module_base.checked_add(spec.rva) else {
        record_outcome(report, PatchOutcome::OutOfBounds);
        return;
    };
    let Some(resume_addr): Option<usize> = site_addr.checked_add(stolen_len) else {
        record_outcome(report, PatchOutcome::OutOfBounds);
        return;
    };

    // SAFETY: `site_addr..site_addr + stolen_len` is bounds-checked inside the mapped image.
    let current: &[u8] = unsafe { std::slice::from_raw_parts(site_addr as *const u8, stolen_len) };
    if current != spec.expected_prologue {
        log_line(&format!("detour '{}': SignatureMismatch", spec.name));
        record_outcome(report, PatchOutcome::SignatureMismatch);
        return;
    }

    let Ok(stub): Result<[u8; EOS_RECV_GUARD_STUB_LEN], TrampolineError> =
        encode_eos_recv_guard_stub(flag_addr, &spec.expected_prologue, resume_addr)
    else {
        record_outcome(report, PatchOutcome::MemoryError);
        return;
    };
    let Some(stub_addr): Option<usize> = append_stub_to_page(builder, &stub) else {
        record_outcome(report, PatchOutcome::MemoryError);
        return;
    };
    let Ok(site_jmp): Result<[u8; X64_JMP_REL32_LEN], TrampolineError> =
        encode_jmp_rel32(site_addr, stub_addr)
    else {
        record_outcome(report, PatchOutcome::MemoryError);
        return;
    };

    let mut patch_bytes: Vec<u8> = vec![X64_NOP_BYTE; stolen_len];
    patch_bytes[..X64_JMP_REL32_LEN].copy_from_slice(&site_jmp);
    builder.pending_patches.push(PendingSitePatch {
        name: spec.name,
        site_addr,
        patch_bytes,
    });
}

#[cfg(windows)]
fn prepare_standard_detour(
    builder: &mut TrampolinePageBuilder<'_>,
    binding: StandardDetourBinding,
    report: &mut PatchReport,
) {
    let spec: &crate::structs::patch::DetourSpec = binding.spec;
    let stolen_len: usize = spec.expected_prologue.len();
    let Some(end_rva): Option<usize> = spec.rva.checked_add(stolen_len) else {
        record_outcome(report, PatchOutcome::OutOfBounds);
        return;
    };
    if stolen_len < X64_JMP_REL32_LEN || end_rva > builder.image_size {
        record_outcome(report, PatchOutcome::OutOfBounds);
        return;
    }
    let Some(site_addr): Option<usize> = builder.module_base.checked_add(spec.rva) else {
        record_outcome(report, PatchOutcome::OutOfBounds);
        return;
    };
    let Some(resume_addr): Option<usize> = site_addr.checked_add(stolen_len) else {
        record_outcome(report, PatchOutcome::OutOfBounds);
        return;
    };

    // SAFETY: `site_addr..site_addr + stolen_len` is bounds-checked inside the mapped image.
    let current: &[u8] = unsafe { std::slice::from_raw_parts(site_addr as *const u8, stolen_len) };
    if current != spec.expected_prologue {
        log_line(&format!("detour '{}': SignatureMismatch", spec.name));
        record_outcome(report, PatchOutcome::SignatureMismatch);
        return;
    }

    let Ok(relay_stub): Result<[u8; X64_ABS_JMP_LEN], TrampolineError> =
        encode_abs_jmp_stub(binding.handler_addr)
    else {
        record_outcome(report, PatchOutcome::MemoryError);
        return;
    };
    let Ok(orig_stub): Result<Vec<u8>, TrampolineError> =
        encode_original_trampoline(spec.expected_prologue, resume_addr)
    else {
        record_outcome(report, PatchOutcome::MemoryError);
        return;
    };

    let Some(relay_addr): Option<usize> = append_stub_to_page(builder, &relay_stub) else {
        record_outcome(report, PatchOutcome::MemoryError);
        return;
    };
    let Some(orig_addr): Option<usize> = append_stub_to_page(builder, &orig_stub) else {
        record_outcome(report, PatchOutcome::MemoryError);
        return;
    };
    let Ok(site_jmp): Result<[u8; X64_JMP_REL32_LEN], TrampolineError> =
        encode_jmp_rel32(site_addr, relay_addr)
    else {
        record_outcome(report, PatchOutcome::MemoryError);
        return;
    };

    (binding.register_orig)(orig_addr);

    let mut patch_bytes: Vec<u8> = vec![X64_NOP_BYTE; stolen_len];
    patch_bytes[..X64_JMP_REL32_LEN].copy_from_slice(&site_jmp);
    builder.pending_patches.push(PendingSitePatch {
        name: spec.name,
        site_addr,
        patch_bytes,
    });
}

#[cfg(windows)]
fn append_stub_to_page(builder: &mut TrampolinePageBuilder<'_>, stub: &[u8]) -> Option<usize> {
    let aligned_off: usize = align_up(builder.cursor, TRAMPOLINE_SLOT_ALIGN)?;
    let end_off: usize = aligned_off.checked_add(stub.len())?;
    if end_off > builder.page_slice.len() {
        return None;
    }
    builder.page_slice[aligned_off..end_off].copy_from_slice(stub);
    builder.cursor = end_off;
    builder.page_addr.checked_add(aligned_off)
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
        // SAFETY: `buf` is a writable UTF-16 buffer of length `capacity`.
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
    // SAFETY: The first `0x1000` bytes of the main executable image are always mapped readable.
    let header_slice: &[u8] =
        unsafe { std::slice::from_raw_parts(module_base as *const u8, TRAMPOLINE_ALLOC_SIZE) };
    parse_pe_size_of_image(header_slice)
}

#[cfg(windows)]
fn allocate_nearby_rw_page(module_base: usize, image_size: usize, size: usize) -> Option<usize> {
    let mut sys_info: SYSTEM_INFO = SYSTEM_INFO::default();
    // SAFETY: `GetSystemInfo` populates `sys_info`.
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
    // SAFETY: Temporarily marks verified code address writable, writes exact bytes, restores
    // original protection immediately, and flushes the instruction cache.
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
    // SAFETY: Restores the original protection (`PAGE_EXECUTE_READ`) so no `RWX` remains.
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
