//! Safe x86-64 nearby `RX` trampoline allocation and 5-byte `jmp rel32` detour helpers.

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
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
use crate::consts::patch::X64_INT3_BYTE;
use crate::consts::patch::{
    DEFAULT_ALLOC_GRANULARITY, PE_DOS_MAGIC, PE_LFANEW_OFFSET, PE_MIN_DOS_BYTES, PE_MIN_NT_BYTES,
    PE_NT_SIGNATURE, PE_OPTIONAL_HEADER_OFFSET, PE_OPT_MAGIC_PE32_PLUS, PE_SIZE_OF_IMAGE_OFFSET,
    TARGET_PROCESS_NAME, TRAMPOLINE_ALLOC_SIZE, TRAMPOLINE_MAX_DISTANCE, X64_JMP_ABS_INDIRECT_LEN,
    X64_JMP_ABS_INDIRECT_PREFIX, X64_JMP_REL32_LEN, X64_JMP_REL32_OPCODE, X64_NOP_BYTE,
};
use crate::structs::patch::DetourError;

/// Checks whether a full executable path or filename matches `DreadHungerServer-Win64-Shipping.exe`
/// case-insensitively.
#[must_use]
pub fn is_target_process_path(path: &str) -> bool {
    let trimmed: &str = path.trim_end_matches('\0');
    let basename: &str = trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed);
    basename.eq_ignore_ascii_case(TARGET_PROCESS_NAME)
}

/// Parses `SizeOfImage` from an in-memory PE32+ header slice.
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

/// Encodes a 5-byte x86-64 `jmp rel32` instruction (`E9 <rel32>`) from `jmp_addr` to `target_addr`.
pub fn encode_jmp_rel32(
    jmp_addr: usize,
    target_addr: usize,
) -> Result<[u8; X64_JMP_REL32_LEN], DetourError> {
    let next_rip: usize = jmp_addr
        .checked_add(X64_JMP_REL32_LEN)
        .ok_or(DetourError::AddressOverflow)?;
    let delta: i128 = i128::try_from(target_addr)
        .map_err(|_err: core::num::TryFromIntError| DetourError::AddressOverflow)?
        - i128::try_from(next_rip)
            .map_err(|_err: core::num::TryFromIntError| DetourError::AddressOverflow)?;
    let rel32: i32 = i32::try_from(delta)
        .map_err(|_err: core::num::TryFromIntError| DetourError::DisplacementOutOfRange)?;
    let rel_bytes: [u8; 4] = rel32.to_le_bytes();
    Ok([
        X64_JMP_REL32_OPCODE,
        rel_bytes[0],
        rel_bytes[1],
        rel_bytes[2],
        rel_bytes[3],
    ])
}

/// Encodes a 14-byte x86-64 absolute indirect jump `jmp qword ptr [rip + 0]; dq target_addr`.
///
/// Works across the entire 64-bit address space without clobbering any register or stack slot.
pub fn encode_jmp_abs_indirect(
    target_addr: usize,
) -> Result<[u8; X64_JMP_ABS_INDIRECT_LEN], DetourError> {
    let target_u64: u64 = u64::try_from(target_addr)
        .map_err(|_err: core::num::TryFromIntError| DetourError::AddressOverflow)?;
    let mut bytes: [u8; X64_JMP_ABS_INDIRECT_LEN] = [0u8; X64_JMP_ABS_INDIRECT_LEN];
    bytes[..X64_JMP_ABS_INDIRECT_PREFIX.len()].copy_from_slice(&X64_JMP_ABS_INDIRECT_PREFIX);
    bytes[X64_JMP_ABS_INDIRECT_PREFIX.len()..].copy_from_slice(&target_u64.to_le_bytes());
    Ok(bytes)
}

/// Encodes a site patch buffer consisting of `jmp rel32` followed by `NOP` padding up to `site_len`.
pub fn encode_detour_site_patch(
    site_addr: usize,
    bridge_addr: usize,
    site_len: usize,
) -> Result<Vec<u8>, DetourError> {
    if site_len < X64_JMP_REL32_LEN {
        return Err(DetourError::OutOfBounds);
    }
    let jmp: [u8; X64_JMP_REL32_LEN] = encode_jmp_rel32(site_addr, bridge_addr)?;
    let mut patch: Vec<u8> = vec![X64_NOP_BYTE; site_len];
    patch[..X64_JMP_REL32_LEN].copy_from_slice(&jmp);
    Ok(patch)
}

/// Builds an original-function trampoline consisting of the copied RIP-independent prologue bytes
/// followed by a 14-byte absolute jump back to `target_addr + stolen_prologue.len()`.
pub fn encode_original_trampoline(
    stolen_prologue: &[u8],
    resume_addr: usize,
) -> Result<Vec<u8>, DetourError> {
    let back_jmp: [u8; X64_JMP_ABS_INDIRECT_LEN] = encode_jmp_abs_indirect(resume_addr)?;
    let mut out: Vec<u8> = Vec::with_capacity(stolen_prologue.len() + back_jmp.len());
    out.extend_from_slice(stolen_prologue);
    out.extend_from_slice(&back_jmp);
    Ok(out)
}

/// Reads `SizeOfImage` from a live PE module base in the current process.
#[cfg(windows)]
#[must_use]
pub fn read_live_image_size(module_base: usize) -> Option<usize> {
    if module_base == 0 {
        return None;
    }
    // SAFETY: The first `0x1000` bytes of the main executable module are always mapped and readable.
    let header_slice: &[u8] =
        unsafe { std::slice::from_raw_parts(module_base as *const u8, TRAMPOLINE_ALLOC_SIZE) };
    parse_pe_size_of_image(header_slice)
}

/// Allocates a private `PAGE_READWRITE` page within `+/- TRAMPOLINE_MAX_DISTANCE` of `module_base`.
#[cfg(windows)]
#[must_use]
pub fn allocate_nearby_rw_page(module_base: usize, image_size: usize) -> Option<usize> {
    let mut sys_info: SYSTEM_INFO = SYSTEM_INFO::default();
    // SAFETY: `GetSystemInfo` writes into our mutable `SYSTEM_INFO` struct.
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

    if let Some(addr) = search_free_slot_ascending(
        search_after_start,
        search_after_limit,
        granularity,
        TRAMPOLINE_ALLOC_SIZE,
    ) {
        return Some(addr);
    }

    let search_before_floor: usize = module_base
        .saturating_sub(TRAMPOLINE_MAX_DISTANCE)
        .max(min_app_addr);
    let search_before_start: usize = align_down(module_base, granularity)?;
    search_free_slot_descending(
        search_before_start,
        search_before_floor,
        granularity,
        TRAMPOLINE_ALLOC_SIZE,
    )
}

/// Seals a freshly populated `RW` trampoline page as `PAGE_EXECUTE_READ` (zero persistent `RWX`)
/// and flushes the CPU instruction cache.
#[cfg(windows)]
pub fn seal_trampoline_page_rx(page_addr: usize, payload: &[u8]) -> Result<(), DetourError> {
    if page_addr == 0 || payload.len() > TRAMPOLINE_ALLOC_SIZE {
        return Err(DetourError::MemoryError);
    }

    // SAFETY: `page_addr` is a freshly committed `PAGE_READWRITE` page of `TRAMPOLINE_ALLOC_SIZE`
    // bytes owned exclusively by this crate.
    unsafe {
        let page_slice: &mut [u8] =
            std::slice::from_raw_parts_mut(page_addr as *mut u8, TRAMPOLINE_ALLOC_SIZE);
        page_slice.fill(X64_INT3_BYTE);
        page_slice[..payload.len()].copy_from_slice(payload);
    }

    let mut old_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Transitions the private trampoline page from `RW` to `RX` (no persistent `RWX`).
    let protect_res: Result<(), windows::core::Error> = unsafe {
        VirtualProtect(
            page_addr as *const c_void,
            TRAMPOLINE_ALLOC_SIZE,
            PAGE_EXECUTE_READ,
            &raw mut old_protect,
        )
    };
    if protect_res.is_err() {
        return Err(DetourError::MemoryError);
    }

    // SAFETY: Flushes CPU instruction caches for our newly executable trampoline page.
    let _flushed: Result<(), windows::core::Error> = unsafe {
        FlushInstructionCache(
            GetCurrentProcess(),
            Some(page_addr as *const c_void),
            TRAMPOLINE_ALLOC_SIZE,
        )
    };
    Ok(())
}

/// Temporarily switches target code protection to `PAGE_EXECUTE_READWRITE`, writes `replacement`,
/// immediately restores the original page protection, and flushes the instruction cache.
#[cfg(windows)]
pub fn write_protected_code(target_addr: usize, replacement: &[u8]) -> Result<(), DetourError> {
    let mut old_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: `target_addr..target_addr + replacement.len()` was bounds-checked and signature-
    // verified against the target process image prior to calling this helper.
    let protect_res: Result<(), windows::core::Error> = unsafe {
        VirtualProtect(
            target_addr as *const c_void,
            replacement.len(),
            PAGE_EXECUTE_READWRITE,
            &raw mut old_protect,
        )
    };
    if protect_res.is_err() {
        return Err(DetourError::MemoryError);
    }

    // SAFETY: The target range is temporarily writable for `replacement.len()` bytes.
    unsafe {
        std::ptr::copy_nonoverlapping(
            replacement.as_ptr(),
            target_addr as *mut u8,
            replacement.len(),
        );
    }

    let mut ignored_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Immediately restores the original page protection (`RX`) so no persistent `RWX` remains.
    let _restore_res: Result<(), windows::core::Error> = unsafe {
        VirtualProtect(
            target_addr as *const c_void,
            replacement.len(),
            old_protect,
            &raw mut ignored_protect,
        )
    };

    // SAFETY: Ensures CPU instruction prefetch observes the newly written branch instruction.
    let _flushed: Result<(), windows::core::Error> = unsafe {
        FlushInstructionCache(
            GetCurrentProcess(),
            Some(target_addr as *const c_void),
            replacement.len(),
        )
    };
    Ok(())
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
        // SAFETY: `VirtualQuery` is read-only and populates `mbi`.
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
        // SAFETY: `VirtualQuery` is read-only and populates `mbi`.
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
    // SAFETY: Requests a fresh private `PAGE_READWRITE` page at a free virtual address.
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
