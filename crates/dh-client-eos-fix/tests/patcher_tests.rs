use std::fs;
use std::path::PathBuf;

use dh_client_eos_fix::consts::patch::{
    ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE, ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN,
    DIRECT_PATCHES, OWNER_RESET_TRAMPOLINES, OWNER_RESET_TRAMPOLINE_LEN, PE_DOS_MAGIC,
    PE_LFANEW_OFFSET, PE_NT_SIGNATURE, PE_OPTIONAL_HEADER_OFFSET, PE_OPT_MAGIC_PE32_PLUS,
    PE_SIZE_OF_IMAGE_OFFSET, POST_UPDATE_EMPTY_URI_RESET, POST_UPDATE_TRAMPOLINE_LEN,
    SET_URI_START_TIME_TRAMPOLINE, SET_URI_START_TIME_TRAMPOLINE_LEN, TOTAL_PATCH_COUNT,
    X64_CALL_MEM_REL32_LEN, X64_CALL_REL32_LEN, X64_CALL_REL32_OPCODE, X64_JMP_MEM_REL32_OPCODES,
};
use dh_client_eos_fix::consts::proxy::{DSOUND_EXPORTS, DSOUND_EXPORT_COUNT};
use dh_client_eos_fix::{
    align_down, align_up, apply_direct_patch_in_buffer,
    encode_add_lobby_attribute_private_uri_trampoline, encode_call_rel32,
    encode_jmp_indirect_rel32, encode_owner_reset_trampoline,
    encode_post_update_empty_uri_trampoline, encode_set_uri_start_time_trampoline,
    is_target_process_path, parse_pe_size_of_image, BytePatch, PatchOutcome, TrampolineError,
};

#[test]
fn target_process_name_matching_is_case_insensitive_and_strict() {
    assert!(is_target_process_path("DreadHunger-Win64-Shipping.exe"));
    assert!(is_target_process_path("dreadhunger-win64-shipping.exe"));
    assert!(is_target_process_path(
        r"DreadHunger\Binaries\Win64\DreadHunger-Win64-Shipping.exe"
    ));
    assert!(is_target_process_path(
        "DreadHunger/Binaries/Win64/DREADHUNGER-WIN64-SHIPPING.EXE\0"
    ));

    assert!(!is_target_process_path(
        "DreadHungerServer-Win64-Shipping.exe"
    ));
    assert!(!is_target_process_path("DH_ServerTool_JA.exe"));
    assert!(!is_target_process_path("notepad.exe"));
    assert!(!is_target_process_path(""));
}

#[test]
fn direct_patch_definitions_have_consistent_lengths_and_distinct_bytes() {
    for patch in &DIRECT_PATCHES {
        assert_eq!(
            patch.expected.len(),
            patch.replacement.len(),
            "length mismatch for {}",
            patch.name
        );
        assert!(
            !patch.expected.is_empty(),
            "empty expected slice for {}",
            patch.name
        );
        assert_ne!(
            patch.expected, patch.replacement,
            "no-op patch for {}",
            patch.name
        );
    }
}

#[test]
fn direct_patch_buffer_verifies_applies_and_is_idempotent() {
    let sample_patch: BytePatch = BytePatch {
        name: "test_patch",
        rva: 16,
        expected: &[0x0F, 0x9E, 0xC0],
        replacement: &[0x30, 0xC0, 0x90],
    };

    let mut buf: Vec<u8> = vec![0x90u8; 64];
    buf[16..19].copy_from_slice(sample_patch.expected);

    assert_eq!(
        apply_direct_patch_in_buffer(&mut buf, &sample_patch),
        PatchOutcome::Applied
    );
    assert_eq!(&buf[16..19], sample_patch.replacement);

    assert_eq!(
        apply_direct_patch_in_buffer(&mut buf, &sample_patch),
        PatchOutcome::AlreadyApplied
    );
}

#[test]
fn direct_patch_skips_on_mismatch_or_out_of_bounds_without_mutating() {
    let sample_patch: BytePatch = BytePatch {
        name: "test_patch",
        rva: 8,
        expected: &[0x85, 0xC0],
        replacement: &[0x0C, 0x01],
    };

    let mut mismatched: Vec<u8> = vec![0xAAu8; 32];
    mismatched[8] = 0x85;
    mismatched[9] = 0xC9;
    let snapshot: Vec<u8> = mismatched.clone();

    assert_eq!(
        apply_direct_patch_in_buffer(&mut mismatched, &sample_patch),
        PatchOutcome::SignatureMismatch
    );
    assert_eq!(mismatched, snapshot);

    let mut tiny: Vec<u8> = vec![0x00u8; 9];
    assert_eq!(
        apply_direct_patch_in_buffer(&mut tiny, &sample_patch),
        PatchOutcome::OutOfBounds
    );
}

#[test]
fn all_trampoline_expected_call_bytes_match_target_rvas() {
    let base: usize = 0x1_4000_0000;

    let post_update_encoded: [u8; X64_CALL_REL32_LEN] = encode_call_rel32(
        base + POST_UPDATE_EMPTY_URI_RESET.call_site_rva,
        base + POST_UPDATE_EMPTY_URI_RESET.original_get_rva,
    )
    .unwrap();
    assert_eq!(
        post_update_encoded,
        POST_UPDATE_EMPTY_URI_RESET.expected_call_bytes
    );

    for spec in &OWNER_RESET_TRAMPOLINES {
        let encoded: [u8; X64_CALL_REL32_LEN] =
            encode_call_rel32(base + spec.call_site_rva, base + spec.is_session_owner_rva).unwrap();
        assert_eq!(
            encoded, spec.expected_call_bytes,
            "call bytes mismatch for {}",
            spec.name
        );
    }

    let set_uri_encoded: [u8; X64_CALL_REL32_LEN] = encode_call_rel32(
        base + SET_URI_START_TIME_TRAMPOLINE.call_site_rva,
        base + SET_URI_START_TIME_TRAMPOLINE.update_session_rva,
    )
    .unwrap();
    assert_eq!(
        set_uri_encoded,
        SET_URI_START_TIME_TRAMPOLINE.expected_call_bytes
    );

    let priv_call_bytes: [u8; X64_CALL_MEM_REL32_LEN] =
        ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE.expected_call_bytes;
    assert_eq!(&priv_call_bytes[0..2], &[0xFF, 0x15]);
    let priv_rel32: i32 = i32::from_le_bytes([
        priv_call_bytes[2],
        priv_call_bytes[3],
        priv_call_bytes[4],
        priv_call_bytes[5],
    ]);
    let resolved_iat: usize = (ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE.call_site_rva
        + X64_CALL_MEM_REL32_LEN)
        .wrapping_add_signed(isize::try_from(priv_rel32).unwrap());
    assert_eq!(
        resolved_iat,
        ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE.iat_slot_rva
    );

    let too_far: usize = base + 0x0001_0000_0000;
    assert_eq!(
        encode_call_rel32(base, too_far),
        Err(TrampolineError::DisplacementOutOfRange)
    );
    assert_eq!(
        encode_jmp_indirect_rel32(base, too_far),
        Err(TrampolineError::DisplacementOutOfRange)
    );
}

#[test]
fn total_patch_count_matches_active_feature_plan() {
    #[cfg(not(feature = "private-uri"))]
    assert_eq!(TOTAL_PATCH_COUNT, 7);

    #[cfg(feature = "private-uri")]
    assert_eq!(TOTAL_PATCH_COUNT, 8);
}

#[test]
fn encoded_trampolines_resolve_all_internal_rel32_callees() {
    let base: usize = 0x1_4000_0000;
    let tramp: usize = base + 0x0600_0000;
    let get_fn: usize = base + POST_UPDATE_EMPTY_URI_RESET.original_get_rva;
    let get_gi: usize = base + POST_UPDATE_EMPTY_URI_RESET.get_game_instance_rva;
    let reset_uri: usize = base + POST_UPDATE_EMPTY_URI_RESET.reset_uri_rva;
    let is_owner: usize = base + OWNER_RESET_TRAMPOLINES[0].is_session_owner_rva;

    let pu_stub: [u8; POST_UPDATE_TRAMPOLINE_LEN] =
        encode_post_update_empty_uri_trampoline(tramp, get_fn, get_gi, reset_uri).unwrap();
    assert_eq!(pu_stub[0x06], X64_CALL_REL32_OPCODE);
    assert_eq!(pu_stub[0x21], X64_CALL_REL32_OPCODE);
    assert_eq!(pu_stub[0x2E], X64_CALL_REL32_OPCODE);

    let owner_stub: [u8; OWNER_RESET_TRAMPOLINE_LEN] =
        encode_owner_reset_trampoline(tramp, is_owner, get_gi, reset_uri).unwrap();
    assert_eq!(owner_stub[0x04], X64_CALL_REL32_OPCODE);
    assert_eq!(owner_stub[0x10], X64_CALL_REL32_OPCODE);
    assert_eq!(owner_stub[0x1D], X64_CALL_REL32_OPCODE);

    let utc_now: usize = base + SET_URI_START_TIME_TRAMPOLINE.utc_now_rva;
    let update_session: usize = base + SET_URI_START_TIME_TRAMPOLINE.update_session_rva;
    let set_uri_stub: [u8; SET_URI_START_TIME_TRAMPOLINE_LEN] =
        encode_set_uri_start_time_trampoline(tramp, utc_now, update_session).unwrap();
    assert_eq!(set_uri_stub[0x17], X64_CALL_REL32_OPCODE);
    assert_eq!(set_uri_stub[0x51], X64_CALL_REL32_OPCODE);

    let iat_slot: usize = base + ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE.iat_slot_rva;
    let priv_stub: [u8; ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN] =
        encode_add_lobby_attribute_private_uri_trampoline(tramp, iat_slot).unwrap();
    assert_eq!(&priv_stub[0x45..0x47], &X64_JMP_MEM_REL32_OPCODES);
    let disp: i32 = i32::from_le_bytes([
        priv_stub[0x47],
        priv_stub[0x48],
        priv_stub[0x49],
        priv_stub[0x4A],
    ]);
    let resolved_iat: usize =
        (tramp + 0x45 + X64_CALL_MEM_REL32_LEN).wrapping_add_signed(isize::try_from(disp).unwrap());
    assert_eq!(resolved_iat, iat_slot);
}

#[test]
fn parse_pe_size_of_image_handles_valid_and_invalid_headers() {
    let mut header: Vec<u8> = vec![0u8; 0x200];
    header[0..2].copy_from_slice(&PE_DOS_MAGIC.to_le_bytes());
    let lfanew: usize = 0x80;
    header[PE_LFANEW_OFFSET..PE_LFANEW_OFFSET + 4]
        .copy_from_slice(&u32::try_from(lfanew).unwrap().to_le_bytes());
    header[lfanew..lfanew + 4].copy_from_slice(&PE_NT_SIGNATURE.to_le_bytes());
    let opt: usize = lfanew + PE_OPTIONAL_HEADER_OFFSET;
    header[opt..opt + 2].copy_from_slice(&PE_OPT_MAGIC_PE32_PLUS.to_le_bytes());
    let size_off: usize = opt + PE_SIZE_OF_IMAGE_OFFSET;
    header[size_off..size_off + 4].copy_from_slice(&0x0500_0000u32.to_le_bytes());

    assert_eq!(parse_pe_size_of_image(&header), Some(0x0500_0000));

    header[0] = 0x00;
    assert_eq!(parse_pe_size_of_image(&header), None);
}

#[test]
fn alignment_helpers_and_export_table_invariants_hold() {
    assert_eq!(align_up(0x1000, 0x10000), Some(0x10000));
    assert_eq!(align_up(0x10000, 0x10000), Some(0x10000));
    assert_eq!(align_down(0x1FFFF, 0x10000), Some(0x10000));
    assert_eq!(align_up(1, 0), None);
    assert_eq!(align_down(1, 0), None);

    assert_eq!(DSOUND_EXPORTS.len(), DSOUND_EXPORT_COUNT);
    for (idx, spec) in DSOUND_EXPORTS.iter().enumerate() {
        assert_eq!(spec.index, idx);
        assert_eq!(usize::from(spec.ordinal), idx + 1);
        assert_eq!(spec.c_name.to_str().unwrap(), spec.name);
    }
}

/// Optional read-only check against the local reference client binary when present in the
/// parent workspace (never modifies `references/` and cleanly skips if absent, e.g. in CI).
#[test]
fn reference_client_binary_matches_all_expected_patch_signatures_if_present() {
    let manifest_dir: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ref_exe: PathBuf = manifest_dir
        .join("..")
        .join("..")
        .join("..")
        .join("references")
        .join("WindowsClient")
        .join("DreadHunger-Win64-Shipping.exe");

    if !ref_exe.is_file() {
        return;
    }

    let raw: Vec<u8> = fs::read(&ref_exe).expect("read reference client exe");
    let lfanew: usize = u32::from_le_bytes(
        raw[PE_LFANEW_OFFSET..PE_LFANEW_OFFSET + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let num_sections: usize = usize::from(u16::from_le_bytes(
        raw[lfanew + 6..lfanew + 8].try_into().unwrap(),
    ));
    let opt_size: usize = usize::from(u16::from_le_bytes(
        raw[lfanew + 20..lfanew + 22].try_into().unwrap(),
    ));
    let sec_table: usize = lfanew + 24 + opt_size;

    let mut sections: Vec<(usize, usize, usize, usize)> = Vec::with_capacity(num_sections);
    for i in 0..num_sections {
        let off: usize = sec_table + i * 40;
        let vsize: usize = u32::from_le_bytes(raw[off + 8..off + 12].try_into().unwrap()) as usize;
        let vaddr: usize = u32::from_le_bytes(raw[off + 12..off + 16].try_into().unwrap()) as usize;
        let rsize: usize = u32::from_le_bytes(raw[off + 16..off + 20].try_into().unwrap()) as usize;
        let rptr: usize = u32::from_le_bytes(raw[off + 20..off + 24].try_into().unwrap()) as usize;
        sections.push((vaddr, vsize, rptr, rsize));
    }

    let rva_to_file = |rva: usize, len: usize| -> Option<&[u8]> {
        for &(vaddr, vsize, rptr, rsize) in &sections {
            let span: usize = vsize.max(rsize);
            if rva >= vaddr && rva + len <= vaddr + span {
                let fo: usize = rptr + (rva - vaddr);
                return raw.get(fo..fo + len);
            }
        }
        None
    };

    for patch in &DIRECT_PATCHES {
        let actual: &[u8] = rva_to_file(patch.rva, patch.expected.len())
            .unwrap_or_else(|| panic!("RVA {:#x} not mapped for {}", patch.rva, patch.name));
        assert_eq!(
            actual, patch.expected,
            "byte signature mismatch for {} at RVA {:#x}",
            patch.name, patch.rva
        );
    }

    let pu_bytes: &[u8] = rva_to_file(
        POST_UPDATE_EMPTY_URI_RESET.call_site_rva,
        X64_CALL_REL32_LEN,
    )
    .expect("PostUpdate call site mapped");
    assert_eq!(pu_bytes, &POST_UPDATE_EMPTY_URI_RESET.expected_call_bytes);

    for spec in &OWNER_RESET_TRAMPOLINES {
        let actual: &[u8] =
            rva_to_file(spec.call_site_rva, X64_CALL_REL32_LEN).unwrap_or_else(|| {
                panic!(
                    "call site RVA {:#x} not mapped for {}",
                    spec.call_site_rva, spec.name
                )
            });
        assert_eq!(
            actual, &spec.expected_call_bytes,
            "call signature mismatch for {}",
            spec.name
        );
    }

    let set_uri_bytes: &[u8] = rva_to_file(
        SET_URI_START_TIME_TRAMPOLINE.call_site_rva,
        X64_CALL_REL32_LEN,
    )
    .expect("SetURI call site mapped");
    assert_eq!(
        set_uri_bytes,
        &SET_URI_START_TIME_TRAMPOLINE.expected_call_bytes
    );

    let priv_uri_bytes: &[u8] = rva_to_file(
        ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE.call_site_rva,
        X64_CALL_MEM_REL32_LEN,
    )
    .expect("AddLobbyAttribute call site mapped");
    assert_eq!(
        priv_uri_bytes,
        &ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE.expected_call_bytes
    );
}

#[cfg(all(windows, target_arch = "x86_64"))]
fn alloc_nearby_rx_page(anchor_fn: usize, stub: &[u8]) -> *mut std::ffi::c_void {
    use std::ffi::c_void;
    use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
    use windows::Win32::System::Memory::{
        VirtualAlloc, VirtualProtect, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READ,
        PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    let mut page_ptr: *mut c_void = std::ptr::null_mut();
    let base_probe: usize = anchor_fn & !0xFFFFusize;
    for step in 1usize..4096usize {
        for candidate in [
            base_probe.wrapping_add(step * 0x10000),
            base_probe.wrapping_sub(step * 0x10000),
        ] {
            // SAFETY: Requesting a fresh private `PAGE_READWRITE` page near the test binary.
            let p: *mut c_void = unsafe {
                VirtualAlloc(
                    Some(candidate as *const c_void),
                    0x1000,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_READWRITE,
                )
            };
            if !p.is_null() {
                page_ptr = p;
                break;
            }
        }
        if !page_ptr.is_null() {
            break;
        }
    }
    assert!(!page_ptr.is_null(), "failed to allocate nearby test page");

    // SAFETY: `page_ptr` is a freshly committed 4 KiB writable page.
    unsafe {
        std::ptr::copy_nonoverlapping(stub.as_ptr(), page_ptr.cast::<u8>(), stub.len());
    }
    let mut old_prot: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Flipping the test trampoline page from `RW` to `RX`.
    let (prot_res, flush_res) = unsafe {
        (
            VirtualProtect(page_ptr, 0x1000, PAGE_EXECUTE_READ, &raw mut old_prot),
            FlushInstructionCache(GetCurrentProcess(), Some(page_ptr), 0x1000),
        )
    };
    assert!(prot_res.is_ok(), "VirtualProtect RX failed: {prot_res:?}");
    assert!(
        flush_res.is_ok(),
        "FlushInstructionCache failed: {flush_res:?}"
    );
    page_ptr
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn post_update_trampoline_resets_only_when_session_uri_is_empty_and_preserves_return() {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
    use windows::Win32::System::Memory::{VirtualFree, MEM_RELEASE};

    #[repr(C)]
    struct MockFString {
        data: usize,
        array_num: i32,
        array_max: i32,
    }

    static NEXT_GET_FOUND: AtomicBool = AtomicBool::new(false);
    static NEXT_URI_LEN: AtomicI32 = AtomicI32::new(0);
    static RESET_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_SESSION_CTX: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_GI: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn mock_get_setting(_settings: usize, _key: usize, out_str: *mut MockFString) -> u8 {
        // SAFETY: `out_str` points to a valid `MockFString` provided by the test harness.
        unsafe {
            (*out_str).array_num = NEXT_URI_LEN.load(Ordering::SeqCst);
        }
        u8::from(NEXT_GET_FOUND.load(Ordering::SeqCst))
    }

    extern "C" fn mock_get_gi(ctx: usize) -> usize {
        CAPTURED_SESSION_CTX.store(ctx, Ordering::SeqCst);
        0x7777_8888usize
    }

    extern "C" fn mock_reset_uri(gi: usize) {
        CAPTURED_GI.store(gi, Ordering::SeqCst);
        RESET_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    let get_addr: usize = mock_get_setting as *const () as usize;
    let gi_addr: usize = mock_get_gi as *const () as usize;
    let reset_addr: usize = mock_reset_uri as *const () as usize;

    // Reserve page first so trampoline_addr is exact, then encode and protect.
    let placeholder: [u8; POST_UPDATE_TRAMPOLINE_LEN] = [0xCCu8; POST_UPDATE_TRAMPOLINE_LEN];
    let page_ptr: *mut c_void = alloc_nearby_rx_page(get_addr, &placeholder);
    let tramp_addr: usize = page_ptr as usize;
    let stub: [u8; POST_UPDATE_TRAMPOLINE_LEN] =
        encode_post_update_empty_uri_trampoline(tramp_addr, get_addr, gi_addr, reset_addr)
            .expect("encode post_update trampoline");
    // Re-write with real rel32 displacements.
    {
        use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
        use windows::Win32::System::Memory::{
            VirtualProtect, PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
        };
        use windows::Win32::System::Threading::GetCurrentProcess;
        let mut old_prot: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
        // SAFETY: Updating our private test page before restoring `RX`.
        unsafe {
            VirtualProtect(page_ptr, 0x1000, PAGE_READWRITE, &raw mut old_prot).unwrap();
            std::ptr::copy_nonoverlapping(stub.as_ptr(), page_ptr.cast::<u8>(), stub.len());
            VirtualProtect(page_ptr, 0x1000, PAGE_EXECUTE_READ, &raw mut old_prot).unwrap();
            FlushInstructionCache(GetCurrentProcess(), Some(page_ptr), 0x1000).unwrap();
        }
    }

    let invoke = |found: bool, uri_len: i32| -> u8 {
        NEXT_GET_FOUND.store(found, Ordering::SeqCst);
        NEXT_URI_LEN.store(uri_len, Ordering::SeqCst);
        let mut out_uri: MockFString = MockFString {
            data: 0,
            array_num: 0,
            array_max: 0,
        };
        let session_this: usize = 0x1234_5678usize;
        let ret_val: usize;
        // SAFETY: Invokes our verified 61-byte `RX` trampoline with `rsi = session_this`
        // matching `UDH_OnlineSession::PostUpdate` at `0xE8894B`.
        unsafe {
            core::arch::asm!(
                "push rsi",
                "sub rsp, 0x28",
                "mov rsi, r11",
                "call r10",
                "add rsp, 0x28",
                "pop rsi",
                inout("r11") session_this => _,
                inout("r10") tramp_addr => _,
                inout("rcx") 0x1111usize => _,
                inout("rdx") 0x2222usize => _,
                inout("r8") (&raw mut out_uri as usize) => _,
                lateout("rax") ret_val,
                clobber_abi("C"),
            );
        }
        u8::try_from(ret_val & 0xFF).unwrap()
    };

    RESET_CALL_COUNT.store(0, Ordering::SeqCst);

    // Case 1: Active match URI present ("1.2.3.4:7895", len=13) -> MUST NOT reset!
    assert_eq!(invoke(true, 13), 1);
    assert_eq!(RESET_CALL_COUNT.load(Ordering::SeqCst), 0);

    // Case 2: Attribute absent (found=false) -> MUST reset and return 0!
    assert_eq!(invoke(false, 0), 0);
    assert_eq!(RESET_CALL_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(CAPTURED_SESSION_CTX.load(Ordering::SeqCst), 0x1234_5678);
    assert_eq!(CAPTURED_GI.load(Ordering::SeqCst), 0x7777_8888);

    // Case 3: Attribute present but empty ("", ArrayNum=1) -> MUST reset and return 1!
    assert_eq!(invoke(true, 1), 1);
    assert_eq!(RESET_CALL_COUNT.load(Ordering::SeqCst), 2);

    // SAFETY: Release the test page.
    unsafe {
        VirtualFree(page_ptr, 0, MEM_RELEASE).expect("VirtualFree");
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn set_uri_start_time_trampoline_sets_unix_utc_seconds_and_calls_update() {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
    use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
    use windows::Win32::System::Memory::{
        VirtualFree, VirtualProtect, MEM_RELEASE, PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS,
        PAGE_READWRITE,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    #[repr(C)]
    struct MockOnlineSession {
        padding: [u8; 0x48],
        start_utc_date_time: i64,
        sentinel_after: u64,
    }

    const UNIX_EPOCH_TICKS: i64 = 621_355_968_000_000_000;
    const TICKS_PER_SECOND: i64 = 10_000_000;

    static MOCK_UTC_TICKS: AtomicI64 = AtomicI64::new(0);
    static UPDATE_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_UPDATE_THIS: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_START_TIME_IN_UPDATE: AtomicI64 = AtomicI64::new(0);

    extern "C" fn mock_utc_now(out_ticks: *mut i64) -> *mut i64 {
        // SAFETY: `out_ticks` points to our trampoline's 8-byte stack slot `[rsp+0x20]`.
        unsafe {
            *out_ticks = MOCK_UTC_TICKS.load(Ordering::SeqCst);
        }
        out_ticks
    }

    extern "C" fn mock_update_session(session: *const MockOnlineSession) {
        CAPTURED_UPDATE_THIS.store(session as usize, Ordering::SeqCst);
        // SAFETY: `session` points to our `MockOnlineSession` stack instance.
        let stamped: i64 = unsafe { (*session).start_utc_date_time };
        CAPTURED_START_TIME_IN_UPDATE.store(stamped, Ordering::SeqCst);
        UPDATE_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    let utc_now_addr: usize = mock_utc_now as *const () as usize;
    let update_addr: usize = mock_update_session as *const () as usize;

    let placeholder: [u8; SET_URI_START_TIME_TRAMPOLINE_LEN] =
        [0xCCu8; SET_URI_START_TIME_TRAMPOLINE_LEN];
    let page_ptr: *mut c_void = alloc_nearby_rx_page(utc_now_addr, &placeholder);
    let tramp_addr: usize = page_ptr as usize;
    let stub: [u8; SET_URI_START_TIME_TRAMPOLINE_LEN] =
        encode_set_uri_start_time_trampoline(tramp_addr, utc_now_addr, update_addr)
            .expect("encode set_uri_start_time trampoline");

    let mut old_prot: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Updating our private test page before restoring `RX`.
    unsafe {
        VirtualProtect(page_ptr, 0x1000, PAGE_READWRITE, &raw mut old_prot).unwrap();
        std::ptr::copy_nonoverlapping(stub.as_ptr(), page_ptr.cast::<u8>(), stub.len());
        VirtualProtect(page_ptr, 0x1000, PAGE_EXECUTE_READ, &raw mut old_prot).unwrap();
        FlushInstructionCache(GetCurrentProcess(), Some(page_ptr), 0x1000).unwrap();
    }

    let invoke = |ticks: i64| -> (i64, usize) {
        MOCK_UTC_TICKS.store(ticks, Ordering::SeqCst);
        let mut session: MockOnlineSession = MockOnlineSession {
            padding: [0xA5u8; 0x48],
            start_utc_date_time: 0,
            sentinel_after: 0xDEAD_BEEF_CAFE_BABEu64,
        };
        let session_ptr: usize = (&raw mut session) as usize;
        // SAFETY: Invokes our verified 91-byte `RX` trampoline with `rcx = session_ptr`,
        // matching `UDH_OnlineSession::SetURI` at `0xE7E386`.
        unsafe {
            core::arch::asm!(
                "sub rsp, 0x28",
                "call r10",
                "add rsp, 0x28",
                inout("r10") tramp_addr => _,
                inout("rcx") session_ptr => _,
                clobber_abi("C"),
            );
        }
        assert_eq!(session.sentinel_after, 0xDEAD_BEEF_CAFE_BABEu64);
        assert!(session.padding.iter().all(|b: &u8| *b == 0xA5u8));
        assert_eq!(CAPTURED_UPDATE_THIS.load(Ordering::SeqCst), session_ptr);
        assert_eq!(
            CAPTURED_START_TIME_IN_UPDATE.load(Ordering::SeqCst),
            session.start_utc_date_time
        );
        (
            session.start_utc_date_time,
            UPDATE_CALL_COUNT.load(Ordering::SeqCst),
        )
    };

    UPDATE_CALL_COUNT.store(0, Ordering::SeqCst);

    // Case 1: Normal modern UTC timestamp (2026-11-26T00:00:00Z + 999.9999 ms sub-second fraction).
    let expected_sec: i64 = 1_795_564_800;
    let modern_ticks: i64 = UNIX_EPOCH_TICKS + expected_sec * TICKS_PER_SECOND + 9_999_999;
    assert_eq!(invoke(modern_ticks), (expected_sec, 1));

    // Case 2: Zero ticks (broken clock / default FDateTime) -> clamped to 1.
    assert_eq!(invoke(0), (1, 2));

    // Case 3: Negative ticks -> clamped to 1 without signed overflow.
    assert_eq!(invoke(-123_456_789), (1, 3));

    // Case 4: Exact Unix epoch (1970-01-01T00:00:00Z) -> clamped to 1.
    assert_eq!(invoke(UNIX_EPOCH_TICKS), (1, 4));

    // Case 5: Sub-second post-epoch (quotient == 0) -> clamped to 1.
    assert_eq!(invoke(UNIX_EPOCH_TICKS + 5_000_000), (1, 5));

    // Case 6: Exactly 1 second post-epoch -> 1.
    assert_eq!(invoke(UNIX_EPOCH_TICKS + TICKS_PER_SECOND), (1, 6));

    // Case 7: 2 seconds post-epoch -> 2.
    assert_eq!(invoke(UNIX_EPOCH_TICKS + 2 * TICKS_PER_SECOND), (2, 7));

    // SAFETY: Release the test page.
    unsafe {
        VirtualFree(page_ptr, 0, MEM_RELEASE).expect("VirtualFree");
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn owner_reset_trampoline_resets_only_for_session_owner_and_preserves_return() {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
    use windows::Win32::System::Memory::{
        VirtualFree, VirtualProtect, MEM_RELEASE, PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS,
        PAGE_READWRITE,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    static IS_OWNER: AtomicBool = AtomicBool::new(false);
    static RESET_COUNT: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_THIS: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_GI: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn mock_is_owner() -> u8 {
        u8::from(IS_OWNER.load(Ordering::SeqCst))
    }

    extern "C" fn mock_get_gi(ctx: usize) -> usize {
        CAPTURED_THIS.store(ctx, Ordering::SeqCst);
        0xCAFE_BABEusize
    }

    extern "C" fn mock_reset_uri(gi: usize) {
        CAPTURED_GI.store(gi, Ordering::SeqCst);
        RESET_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    let owner_addr: usize = mock_is_owner as *const () as usize;
    let gi_addr: usize = mock_get_gi as *const () as usize;
    let reset_addr: usize = mock_reset_uri as *const () as usize;

    let placeholder: [u8; OWNER_RESET_TRAMPOLINE_LEN] = [0xCCu8; OWNER_RESET_TRAMPOLINE_LEN];
    let page_ptr: *mut c_void = alloc_nearby_rx_page(owner_addr, &placeholder);
    let tramp_addr: usize = page_ptr as usize;
    let stub: [u8; OWNER_RESET_TRAMPOLINE_LEN] =
        encode_owner_reset_trampoline(tramp_addr, owner_addr, gi_addr, reset_addr)
            .expect("encode owner reset trampoline");

    let mut old_prot: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Updating our private test page before restoring `RX`.
    unsafe {
        VirtualProtect(page_ptr, 0x1000, PAGE_READWRITE, &raw mut old_prot).unwrap();
        std::ptr::copy_nonoverlapping(stub.as_ptr(), page_ptr.cast::<u8>(), stub.len());
        VirtualProtect(page_ptr, 0x1000, PAGE_EXECUTE_READ, &raw mut old_prot).unwrap();
        FlushInstructionCache(GetCurrentProcess(), Some(page_ptr), 0x1000).unwrap();
    }

    let invoke = |owner: bool| -> u8 {
        IS_OWNER.store(owner, Ordering::SeqCst);
        let session_this: usize = 0x5555_6666usize;
        let ret_val: usize;
        // SAFETY: Invokes our verified 41-byte `RX` trampoline with `rdi = session_this`
        // matching `OnMatchEnded` (`0xE81B59`) and `ResetCurrentData` (`0xE8B3A3`).
        unsafe {
            core::arch::asm!(
                "push rdi",
                "sub rsp, 0x28",
                "mov rdi, r11",
                "call r10",
                "add rsp, 0x28",
                "pop rdi",
                inout("r11") session_this => _,
                inout("r10") tramp_addr => _,
                lateout("rax") ret_val,
                clobber_abi("C"),
            );
        }
        u8::try_from(ret_val & 0xFF).unwrap()
    };

    RESET_COUNT.store(0, Ordering::SeqCst);

    // Non-host: must NOT reset prematurely, returns 0.
    assert_eq!(invoke(false), 0);
    assert_eq!(RESET_COUNT.load(Ordering::SeqCst), 0);

    // Session owner: MUST reset via GetInstance(rdi) -> ResetURI and return 1.
    assert_eq!(invoke(true), 1);
    assert_eq!(RESET_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(CAPTURED_THIS.load(Ordering::SeqCst), 0x5555_6666);
    assert_eq!(CAPTURED_GI.load(Ordering::SeqCst), 0xCAFE_BABE);

    // SAFETY: Release the test page.
    unsafe {
        VirtualFree(page_ptr, 0, MEM_RELEASE).expect("VirtualFree");
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn add_lobby_attribute_private_uri_trampoline_hides_only_uri_key_and_preserves_abi() {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
    use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
    use windows::Win32::System::Memory::{
        VirtualFree, VirtualProtect, MEM_RELEASE, PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS,
        PAGE_READWRITE,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    #[repr(C)]
    struct MockEosAttributeData {
        api_version: i32,
        pad0: i32,
        key: *const u8,
        value: *const u8,
        value_type: i32,
        pad1: i32,
    }

    #[repr(C)]
    struct MockEosAddAttributeOptions {
        api_version: i32,
        pad0: i32,
        attribute: *const MockEosAttributeData,
        visibility: i32,
        pad1: i32,
    }

    static MOCK_IAT_SLOT: AtomicUsize = AtomicUsize::new(0);
    static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_HANDLE: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_OPTIONS: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_VISIBILITY: AtomicI32 = AtomicI32::new(-1);

    extern "C" fn mock_eos_add_attribute(
        handle: usize,
        options: *const MockEosAddAttributeOptions,
    ) -> i32 {
        CAPTURED_HANDLE.store(handle, Ordering::SeqCst);
        CAPTURED_OPTIONS.store(options as usize, Ordering::SeqCst);
        if options.is_null() {
            CAPTURED_VISIBILITY.store(-99, Ordering::SeqCst);
        } else {
            // SAFETY: `options` points to our valid stack `MockEosAddAttributeOptions`.
            let vis: i32 = unsafe { (*options).visibility };
            CAPTURED_VISIBILITY.store(vis, Ordering::SeqCst);
        }
        CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        0x1234
    }

    let mock_addr: usize = mock_eos_add_attribute as *const () as usize;
    MOCK_IAT_SLOT.store(mock_addr, Ordering::SeqCst);
    let iat_slot_addr: usize = (&raw const MOCK_IAT_SLOT) as usize;

    let placeholder: [u8; ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN] =
        [0xCCu8; ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN];
    let page_ptr: *mut c_void = alloc_nearby_rx_page(iat_slot_addr, &placeholder);
    let tramp_addr: usize = page_ptr as usize;
    let stub: [u8; ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN] =
        encode_add_lobby_attribute_private_uri_trampoline(tramp_addr, iat_slot_addr)
            .expect("encode private-uri trampoline");

    let mut old_prot: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Updating our private test page before restoring `RX`.
    unsafe {
        VirtualProtect(page_ptr, 0x1000, PAGE_READWRITE, &raw mut old_prot).unwrap();
        std::ptr::copy_nonoverlapping(stub.as_ptr(), page_ptr.cast::<u8>(), stub.len());
        VirtualProtect(page_ptr, 0x1000, PAGE_EXECUTE_READ, &raw mut old_prot).unwrap();
        FlushInstructionCache(GetCurrentProcess(), Some(page_ptr), 0x1000).unwrap();
    }

    let invoke_raw = |handle: usize, options_ptr: usize| -> i32 {
        let ret_val: usize;
        // SAFETY: Invokes our 75-byte `RX` trampoline matching `FOnlineSessionEOS::AddLobbyAttribute`
        // (`0x9CA894`), verifying `rcx`/`rdx` forwarding, return value propagation, and clean
        // callee-saved preservation.
        unsafe {
            core::arch::asm!(
                "sub rsp, 0x28",
                "call r10",
                "add rsp, 0x28",
                inout("r10") tramp_addr => _,
                inout("rcx") handle => _,
                inout("rdx") options_ptr => _,
                lateout("rax") ret_val,
                clobber_abi("C"),
            );
        }
        i32::try_from(ret_val & 0xFFFF_FFFF).unwrap()
    };

    let invoke_key = |key_cstr: Option<&std::ffi::CStr>, expected_vis: i32| {
        let key_ptr: *const u8 = match key_cstr {
            Some(cstr) => cstr.as_ptr().cast::<u8>(),
            None => std::ptr::null(),
        };
        let attr: MockEosAttributeData = MockEosAttributeData {
            api_version: 1,
            pad0: 0,
            key: key_ptr,
            value: c"1.2.3.4:7895".as_ptr().cast::<u8>(),
            value_type: 3,
            pad1: 0,
        };
        let mut opts: MockEosAddAttributeOptions = MockEosAddAttributeOptions {
            api_version: 1,
            pad0: 0,
            attribute: &raw const attr,
            visibility: 0,
            pad1: 0,
        };
        let handle_val: usize = 0x0BAD_F00D_usize;
        let opts_ptr: usize = (&raw mut opts) as usize;
        let ret: i32 = invoke_raw(handle_val, opts_ptr);
        assert_eq!(ret, 0x1234);
        assert_eq!(CAPTURED_HANDLE.load(Ordering::SeqCst), handle_val);
        assert_eq!(CAPTURED_OPTIONS.load(Ordering::SeqCst), opts_ptr);
        assert_eq!(opts.visibility, expected_vis);
        assert_eq!(opts.pad1, 0);
        assert_eq!(CAPTURED_VISIBILITY.load(Ordering::SeqCst), expected_vis);
    };

    CALL_COUNT.store(0, Ordering::SeqCst);

    // Exact "URI" (case-insensitive) -> EOS_LAT_PRIVATE (1).
    invoke_key(Some(c"URI"), 1);
    invoke_key(Some(c"uri"), 1);
    invoke_key(Some(c"UrI"), 1);

    // Other keys / prefix collisions / short strings / empty / NULL -> EOS_LAT_PUBLIC (0).
    invoke_key(Some(c"MEMBERS"), 0);
    invoke_key(Some(c"STARTUTCDATETIME"), 0);
    invoke_key(Some(c"URIX"), 0);
    invoke_key(Some(c"UR"), 0);
    invoke_key(Some(c"U"), 0);
    invoke_key(Some(c""), 0);
    invoke_key(None, 0);

    // NULL Attribute pointer -> stays 0 without crashing.
    let mut null_attr_opts: MockEosAddAttributeOptions = MockEosAddAttributeOptions {
        api_version: 1,
        pad0: 0,
        attribute: std::ptr::null(),
        visibility: 0,
        pad1: 0,
    };
    assert_eq!(
        invoke_raw(0x1111, (&raw mut null_attr_opts) as usize),
        0x1234
    );
    assert_eq!(null_attr_opts.visibility, 0);
    assert_eq!(CAPTURED_VISIBILITY.load(Ordering::SeqCst), 0);

    // NULL Options pointer -> forwarded safely without crashing.
    assert_eq!(invoke_raw(0x2222, 0), 0x1234);
    assert_eq!(CAPTURED_OPTIONS.load(Ordering::SeqCst), 0);
    assert_eq!(CAPTURED_VISIBILITY.load(Ordering::SeqCst), -99);

    assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 12);

    // SAFETY: Release the test page.
    unsafe {
        VirtualFree(page_ptr, 0, MEM_RELEASE).expect("VirtualFree");
    }
}
