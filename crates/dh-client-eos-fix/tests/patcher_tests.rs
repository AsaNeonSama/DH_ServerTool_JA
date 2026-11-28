use std::fs;
use std::path::PathBuf;

use dh_client_eos_fix::consts::patch::{
    ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE, ADD_LOBBY_ATTRIBUTE_PRIVATE_URI_TRAMPOLINE_LEN,
    DIRECT_PATCHES, EOS_LOBBY_MAX_ATTRIBUTE_LEN, ON_DESTROY_CLEAR_STARTED_TRAMPOLINE,
    ON_DESTROY_CLEAR_STARTED_TRAMPOLINE_LEN, ON_JOIN_RESET_URI_TRAMPOLINE,
    ON_JOIN_RESET_URI_TRAMPOLINE_LEN, ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE,
    ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE_LEN, OWNER_RESET_TRAMPOLINES,
    OWNER_RESET_TRAMPOLINE_LEN, PE_DOS_MAGIC, PE_LFANEW_OFFSET, PE_NT_SIGNATURE,
    PE_OPTIONAL_HEADER_OFFSET, PE_OPT_MAGIC_PE32_PLUS, PE_SIZE_OF_IMAGE_OFFSET,
    POST_UPDATE_EMPTY_URI_RESET, POST_UPDATE_TRAMPOLINE_LEN,
    SET_LOBBY_ATTRIBUTES_MAX_LEN_TRAMPOLINE, SET_LOBBY_ATTR_MAX_LEN_TRAMPOLINE_LEN,
    SET_LOBBY_ATTR_SITE_LEN, SET_URI_START_TIME_TRAMPOLINE, SET_URI_START_TIME_TRAMPOLINE_LEN,
    TOTAL_PATCH_COUNT, X64_CALL_MEM_REL32_LEN, X64_CALL_REL32_LEN, X64_CALL_REL32_OPCODE,
    X64_JMP_MEM_REL32_OPCODES, X64_JMP_REL32_LEN, X64_JMP_REL32_OPCODE, X64_MOV_RCX_IMM64_PREFIX,
};
use dh_client_eos_fix::consts::proxy::{DSOUND_EXPORTS, DSOUND_EXPORT_COUNT};
use dh_client_eos_fix::{
    align_down, align_up, apply_direct_patch_in_buffer,
    encode_add_lobby_attribute_private_uri_trampoline, encode_call_rel32,
    encode_jmp_indirect_rel32, encode_jmp_rel32, encode_on_destroy_clear_started_trampoline,
    encode_on_join_reset_uri_trampoline, encode_on_participants_keep_members_trampoline,
    encode_owner_reset_trampoline, encode_post_update_empty_uri_trampoline,
    encode_set_lobby_attributes_max_len_trampoline, encode_set_uri_start_time_trampoline,
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
    assert!(!is_target_process_path("DH_ServerLauncher_JA.exe"));
    assert!(!is_target_process_path("DH_ClientLauncher_JA.exe"));
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

    let destroy_encoded: [u8; X64_CALL_REL32_LEN] = encode_call_rel32(
        base + ON_DESTROY_CLEAR_STARTED_TRAMPOLINE.call_site_rva,
        base + ON_DESTROY_CLEAR_STARTED_TRAMPOLINE.original_callee_rva,
    )
    .unwrap();
    assert_eq!(
        destroy_encoded,
        ON_DESTROY_CLEAR_STARTED_TRAMPOLINE.expected_call_bytes
    );

    let join_encoded: [u8; X64_CALL_REL32_LEN] = encode_call_rel32(
        base + ON_JOIN_RESET_URI_TRAMPOLINE.call_site_rva,
        base + ON_JOIN_RESET_URI_TRAMPOLINE.update_session_rva,
    )
    .unwrap();
    assert_eq!(
        join_encoded,
        ON_JOIN_RESET_URI_TRAMPOLINE.expected_call_bytes
    );

    let participants_encoded: [u8; X64_CALL_REL32_LEN] = encode_call_rel32(
        base + ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE.call_site_rva,
        base + ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE.is_lobby_map_rva,
    )
    .unwrap();
    assert_eq!(
        participants_encoded,
        ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE.expected_call_bytes
    );

    assert_eq!(
        SET_LOBBY_ATTRIBUTES_MAX_LEN_TRAMPOLINE
            .expected_site_bytes
            .len(),
        SET_LOBBY_ATTR_SITE_LEN
    );
    assert_eq!(
        &SET_LOBBY_ATTRIBUTES_MAX_LEN_TRAMPOLINE.expected_site_bytes[20..22],
        &[0xFF, 0x15]
    );

    let too_far: usize = base + 0x0001_0000_0000;
    assert_eq!(
        encode_call_rel32(base, too_far),
        Err(TrampolineError::DisplacementOutOfRange)
    );
    assert_eq!(
        encode_jmp_rel32(base, too_far),
        Err(TrampolineError::DisplacementOutOfRange)
    );
    assert_eq!(
        encode_jmp_indirect_rel32(base, too_far),
        Err(TrampolineError::DisplacementOutOfRange)
    );
    assert_eq!(
        encode_set_lobby_attributes_max_len_trampoline(0),
        Err(TrampolineError::AddressOverflow)
    );
}

#[test]
fn total_patch_count_matches_active_feature_plan() {
    #[cfg(not(feature = "private-uri"))]
    assert_eq!(TOTAL_PATCH_COUNT, 19);

    #[cfg(feature = "private-uri")]
    assert_eq!(TOTAL_PATCH_COUNT, 20);
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
    assert_eq!(owner_stub[0x10], X64_CALL_REL32_OPCODE);
    assert_eq!(owner_stub[0x1C], X64_CALL_REL32_OPCODE);
    assert_eq!(owner_stub[0x29], X64_CALL_REL32_OPCODE);

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

    let rw_buf: usize = 0x0000_0234_5678_9000usize;
    let attr_stub: [u8; SET_LOBBY_ATTR_MAX_LEN_TRAMPOLINE_LEN] =
        encode_set_lobby_attributes_max_len_trampoline(rw_buf).unwrap();
    assert_eq!(&attr_stub[0x00..0x02], &X64_MOV_RCX_IMM64_PREFIX);
    assert_eq!(
        u64::from_le_bytes(attr_stub[0x02..0x0A].try_into().unwrap()),
        u64::try_from(rw_buf).unwrap()
    );
    assert_eq!(attr_stub[SET_LOBBY_ATTR_MAX_LEN_TRAMPOLINE_LEN - 1], 0xC3);

    let resolve_rel32 = |stub: &[u8], offset: usize, expected_opcode: u8| -> usize {
        assert_eq!(stub[offset], expected_opcode);
        let rel: i32 = i32::from_le_bytes([
            stub[offset + 1],
            stub[offset + 2],
            stub[offset + 3],
            stub[offset + 4],
        ]);
        (tramp + offset + X64_JMP_REL32_LEN).wrapping_add_signed(isize::try_from(rel).unwrap())
    };

    let destroy_callee: usize = base + ON_DESTROY_CLEAR_STARTED_TRAMPOLINE.original_callee_rva;
    let destroy_stub: [u8; ON_DESTROY_CLEAR_STARTED_TRAMPOLINE_LEN] =
        encode_on_destroy_clear_started_trampoline(tramp, destroy_callee).unwrap();
    assert_eq!(
        resolve_rel32(&destroy_stub, 0x0C, X64_JMP_REL32_OPCODE),
        destroy_callee
    );

    let join_stub: [u8; ON_JOIN_RESET_URI_TRAMPOLINE_LEN] =
        encode_on_join_reset_uri_trampoline(tramp, get_gi, reset_uri, update_session).unwrap();
    assert_eq!(
        resolve_rel32(&join_stub, 0x09, X64_CALL_REL32_OPCODE),
        get_gi
    );
    assert_eq!(
        resolve_rel32(&join_stub, 0x16, X64_CALL_REL32_OPCODE),
        reset_uri
    );
    assert_eq!(
        resolve_rel32(&join_stub, 0x24, X64_JMP_REL32_OPCODE),
        update_session
    );

    let is_lobby_map: usize = base + ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE.is_lobby_map_rva;
    let participants_stub: [u8; ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE_LEN] =
        encode_on_participants_keep_members_trampoline(tramp, is_lobby_map).unwrap();
    assert_eq!(
        resolve_rel32(&participants_stub, 0x0F, X64_JMP_REL32_OPCODE),
        is_lobby_map
    );
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

    let set_attr_bytes: &[u8] = rva_to_file(
        SET_LOBBY_ATTRIBUTES_MAX_LEN_TRAMPOLINE.site_rva,
        SET_LOBBY_ATTR_SITE_LEN,
    )
    .expect("SetLobbyAttributes strncpy site mapped");
    assert_eq!(
        set_attr_bytes,
        &SET_LOBBY_ATTRIBUTES_MAX_LEN_TRAMPOLINE.expected_site_bytes
    );

    let destroy_bytes: &[u8] = rva_to_file(
        ON_DESTROY_CLEAR_STARTED_TRAMPOLINE.call_site_rva,
        X64_CALL_REL32_LEN,
    )
    .expect("OnDestroySessionComplete call site mapped");
    assert_eq!(
        destroy_bytes,
        &ON_DESTROY_CLEAR_STARTED_TRAMPOLINE.expected_call_bytes
    );

    let join_bytes: &[u8] = rva_to_file(
        ON_JOIN_RESET_URI_TRAMPOLINE.call_site_rva,
        X64_CALL_REL32_LEN,
    )
    .expect("OnJoinSessionComplete call site mapped");
    assert_eq!(
        join_bytes,
        &ON_JOIN_RESET_URI_TRAMPOLINE.expected_call_bytes
    );

    let participants_bytes: &[u8] = rva_to_file(
        ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE.call_site_rva,
        X64_CALL_REL32_LEN,
    )
    .expect("OnSessionParticipantsChange call site mapped");
    assert_eq!(
        participants_bytes,
        &ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE.expected_call_bytes
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

    #[repr(C)]
    struct MockOnlineSessionFlag {
        pad: [u8; 0x16E],
        b_has_game_started: u8,
        sentinel_after: u8,
    }

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

    let invoke = |owner: bool, session_ptr: usize| -> u8 {
        IS_OWNER.store(owner, Ordering::SeqCst);
        let ret_val: usize;
        // SAFETY: Invokes our verified 53-byte `RX` trampoline with `rdi = session_ptr`
        // matching `OnMatchEnded` (`0xE81B59`) and `ResetCurrentData` (`0xE8B3A3`).
        unsafe {
            core::arch::asm!(
                "push rdi",
                "sub rsp, 0x28",
                "mov rdi, r11",
                "call r10",
                "add rsp, 0x28",
                "pop rdi",
                inout("r11") session_ptr => _,
                inout("r10") tramp_addr => _,
                lateout("rax") ret_val,
                clobber_abi("C"),
            );
        }
        u8::try_from(ret_val & 0xFF).unwrap()
    };

    RESET_COUNT.store(0, Ordering::SeqCst);

    // Non-host: clears bHasGameStarted (1 -> 0) but must NOT reset URI prematurely, returns 0.
    let mut non_owner_session: MockOnlineSessionFlag = MockOnlineSessionFlag {
        pad: [0x5Au8; 0x16E],
        b_has_game_started: 1,
        sentinel_after: 0xEE,
    };
    assert_eq!(invoke(false, (&raw mut non_owner_session) as usize), 0);
    assert_eq!(non_owner_session.b_has_game_started, 0);
    assert_eq!(non_owner_session.sentinel_after, 0xEE);
    assert!(non_owner_session.pad.iter().all(|b: &u8| *b == 0x5Au8));
    assert_eq!(RESET_COUNT.load(Ordering::SeqCst), 0);

    // Session owner: clears bHasGameStarted (1 -> 0), resets URI via GetInstance(rdi), returns 1.
    let mut owner_session: MockOnlineSessionFlag = MockOnlineSessionFlag {
        pad: [0xA5u8; 0x16E],
        b_has_game_started: 1,
        sentinel_after: 0xCC,
    };
    let owner_ptr: usize = (&raw mut owner_session) as usize;
    assert_eq!(invoke(true, owner_ptr), 1);
    assert_eq!(owner_session.b_has_game_started, 0);
    assert_eq!(owner_session.sentinel_after, 0xCC);
    assert!(owner_session.pad.iter().all(|b: &u8| *b == 0xA5u8));
    assert_eq!(RESET_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(CAPTURED_THIS.load(Ordering::SeqCst), owner_ptr);
    assert_eq!(CAPTURED_GI.load(Ordering::SeqCst), 0xCAFE_BABE);

    // Null rdi safety check (does not crash on null session pointer).
    assert_eq!(invoke(false, 0), 0);

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

#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn set_lobby_attributes_max_len_trampoline_copies_up_to_1000_utf8_bytes_safely() {
    use std::ffi::{c_void, CStr};
    use windows::Win32::System::Memory::{
        VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
    };

    #[repr(C)]
    struct MockCallerFrame {
        pad: [u8; 0x80],
        value_as_utf8: usize,
        sentinel_after: u64,
    }

    #[repr(C)]
    struct MockFtcharToUtf8 {
        pad: [u8; 0x88],
        utf8_ptr: *const u8,
    }

    // Separate private RW data page (never marked executable) matching live patcher architecture.
    // SAFETY: Allocating a fresh private 4 KiB `PAGE_READWRITE` page for the test copy buffer.
    let rw_buf_ptr: *mut c_void =
        unsafe { VirtualAlloc(None, 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) };
    assert!(!rw_buf_ptr.is_null(), "VirtualAlloc RW buffer failed");
    let rw_buf_addr: usize = rw_buf_ptr as usize;

    let stub: [u8; SET_LOBBY_ATTR_MAX_LEN_TRAMPOLINE_LEN] =
        encode_set_lobby_attributes_max_len_trampoline(rw_buf_addr)
            .expect("encode SetLobbyAttributes max-len trampoline");
    let code_page: *mut c_void = alloc_nearby_rx_page(
        set_lobby_attributes_max_len_trampoline_copies_up_to_1000_utf8_bytes_safely as *const ()
            as usize,
        &stub,
    );
    let tramp_addr: usize = code_page as usize;

    let invoke_raw = |conv_ptr: usize| -> String {
        // Fill destination page with non-zero sentinel bytes to verify proper NUL termination.
        // SAFETY: `rw_buf_ptr` is a 4 KiB writable page owned by this test.
        unsafe {
            std::ptr::write_bytes(rw_buf_ptr.cast::<u8>(), 0xFE, 0x1000);
        }
        let mut frame: MockCallerFrame = MockCallerFrame {
            pad: [0x33u8; 0x80],
            value_as_utf8: 0x1111_2222_3333_4444,
            sentinel_after: 0xDEAD_BEEF_CAFE_BABE,
        };
        let frame_ptr: usize = (&raw mut frame) as usize;
        let ret_rax: usize;
        // SAFETY: Saves/restores `rbp` cleanly around invoking our verified 98-byte `RX` leaf
        // trampoline with `rax = conv_ptr` and `rbp = &frame`, matching `0xA03820`.
        unsafe {
            core::arch::asm!(
                "push rbp",
                "sub rsp, 0x28",
                "mov rbp, r11",
                "call r10",
                "add rsp, 0x28",
                "pop rbp",
                inout("r11") frame_ptr => _,
                inout("r10") tramp_addr => _,
                inout("rax") conv_ptr => ret_rax,
                clobber_abi("C"),
            );
        }
        assert_eq!(ret_rax, rw_buf_addr);
        assert_eq!(frame.value_as_utf8, rw_buf_addr);
        assert_eq!(frame.sentinel_after, 0xDEAD_BEEF_CAFE_BABE);
        assert!(frame.pad.iter().all(|b: &u8| *b == 0x33u8));

        // SAFETY: Trampoline NUL-terminates within `0..=1000` bytes inside the 4 KiB RW page.
        let cstr: &CStr = unsafe { CStr::from_ptr(rw_buf_ptr.cast::<i8>()) };
        let bytes: &[u8] = cstr.to_bytes();
        assert!(bytes.len() <= EOS_LOBBY_MAX_ATTRIBUTE_LEN);
        std::str::from_utf8(bytes)
            .expect("output must always be valid UTF-8")
            .to_owned()
    };

    let invoke_bytes = |nul_terminated: &[u8]| -> String {
        assert_eq!(nul_terminated.last(), Some(&0u8));
        let conv: MockFtcharToUtf8 = MockFtcharToUtf8 {
            pad: [0u8; 0x88],
            utf8_ptr: nul_terminated.as_ptr(),
        };
        invoke_raw((&raw const conv) as usize)
    };

    // 1. Short ASCII string.
    assert_eq!(invoke_bytes(b"1.2.3.4:7895\0"), "1.2.3.4:7895");

    // 2. Empty string.
    assert_eq!(invoke_bytes(b"\0"), "");

    // 3. 300-byte ASCII (> legacy 256B limit!).
    let ascii_300: String = "A".repeat(300);
    let mut buf_300: Vec<u8> = ascii_300.as_bytes().to_vec();
    buf_300.push(0);
    assert_eq!(invoke_bytes(&buf_300), ascii_300);

    // 4. Realistic 8-player Japanese + PUID MEMBERS string (~850 bytes).
    let mut members_8: String = String::new();
    for idx in 1..=8 {
        if idx > 1 {
            members_8.push(',');
        }
        std::fmt::Write::write_fmt(
            &mut members_8,
            format_args!("0002abcdef0123456789abcdef01234{idx}:船員プレイヤー名前_{idx}_テスト"),
        )
        .unwrap();
    }
    assert!(members_8.len() > 400 && members_8.len() < EOS_LOBBY_MAX_ATTRIBUTE_LEN);
    let mut members_buf: Vec<u8> = members_8.as_bytes().to_vec();
    members_buf.push(0);
    assert_eq!(invoke_bytes(&members_buf), members_8);

    // 5. Exact 1000-byte ASCII & 1050-byte ASCII truncation.
    let ascii_1000: String = "B".repeat(1000);
    let mut buf_1000: Vec<u8> = ascii_1000.as_bytes().to_vec();
    buf_1000.push(0);
    assert_eq!(invoke_bytes(&buf_1000), ascii_1000);

    let mut buf_1050: Vec<u8> = "C".repeat(1050).into_bytes();
    buf_1050.push(0);
    assert_eq!(invoke_bytes(&buf_1050), "C".repeat(1000));

    // 6. Complete 3-byte Japanese char ending at byte 1000 (997 + 3 = 1000), followed by another.
    let mut exact_jp: String = "x".repeat(997);
    exact_jp.push('あ');
    let expected_exact_jp: String = exact_jp.clone();
    exact_jp.push('い');
    let mut exact_jp_buf: Vec<u8> = exact_jp.into_bytes();
    exact_jp_buf.push(0);
    assert_eq!(invoke_bytes(&exact_jp_buf), expected_exact_jp);

    // 7. Split 3-byte Japanese char starting at 998 (998, 999, 1000) -> drops split char -> 998B.
    let mut split_998: String = "y".repeat(998);
    split_998.push_str("あい");
    let mut split_998_buf: Vec<u8> = split_998.into_bytes();
    split_998_buf.push(0);
    assert_eq!(invoke_bytes(&split_998_buf), "y".repeat(998));

    // 8. Split 3-byte Japanese char starting at 999 (999, 1000, 1001) -> drops split char -> 999B.
    let mut split_999: String = "z".repeat(999);
    split_999.push('あ');
    let mut split_999_buf: Vec<u8> = split_999.into_bytes();
    split_999_buf.push(0);
    assert_eq!(invoke_bytes(&split_999_buf), "z".repeat(999));

    // 9. Split 4-byte emoji at 997..1000 -> drops split emoji -> 997B.
    let mut split_emoji: String = "e".repeat(997);
    split_emoji.push('😀');
    let mut split_emoji_buf: Vec<u8> = split_emoji.into_bytes();
    split_emoji_buf.push(0);
    assert_eq!(invoke_bytes(&split_emoji_buf), "e".repeat(997));

    // 10. Split 2-byte UTF-8 char at 999..1000 -> drops split char -> 999B.
    let mut split_2b: String = "d".repeat(999);
    split_2b.push('é');
    let mut split_2b_buf: Vec<u8> = split_2b.into_bytes();
    split_2b_buf.push(0);
    assert_eq!(invoke_bytes(&split_2b_buf), "d".repeat(999));

    // 11. Pathological 1001 raw UTF-8 continuation bytes -> safely walks back to 0 without underflow.
    let mut bad_cont: Vec<u8> = vec![0x80u8; 1001];
    bad_cont.push(0);
    assert_eq!(invoke_bytes(&bad_cont), "");

    // 12. NULL FTCHARToUTF8* and NULL utf8_ptr guards.
    assert_eq!(invoke_raw(0), "");
    let null_str_conv: MockFtcharToUtf8 = MockFtcharToUtf8 {
        pad: [0u8; 0x88],
        utf8_ptr: std::ptr::null(),
    };
    assert_eq!(invoke_raw((&raw const null_str_conv) as usize), "");

    // SAFETY: Release both test pages.
    unsafe {
        VirtualFree(code_page, 0, MEM_RELEASE).expect("VirtualFree code_page");
        VirtualFree(rw_buf_ptr, 0, MEM_RELEASE).expect("VirtualFree rw_buf_ptr");
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn on_destroy_clear_started_trampoline_clears_flag_and_tail_calls_callee() {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
    use windows::Win32::System::Memory::{
        VirtualFree, VirtualProtect, MEM_RELEASE, PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS,
        PAGE_READWRITE,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    #[repr(C)]
    struct MockOnlineSession {
        pad: [u8; 0x16E],
        b_has_game_started: u8,
        sentinel_after: u8,
    }

    static CAPTURED_CTX: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_FLAG_AT_CALLEE: AtomicUsize = AtomicUsize::new(99);
    static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn mock_destroy_callee(session: *const MockOnlineSession) -> usize {
        CAPTURED_CTX.store(session as usize, Ordering::SeqCst);
        if !session.is_null() {
            // SAFETY: `session` points to our valid stack `MockOnlineSession`.
            let flag: u8 = unsafe { (*session).b_has_game_started };
            CAPTURED_FLAG_AT_CALLEE.store(usize::from(flag), Ordering::SeqCst);
        }
        CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        if session.is_null() {
            0
        } else {
            0xDEAD_1234usize
        }
    }

    let callee_addr: usize = mock_destroy_callee as *const () as usize;
    let placeholder: [u8; ON_DESTROY_CLEAR_STARTED_TRAMPOLINE_LEN] =
        [0xCCu8; ON_DESTROY_CLEAR_STARTED_TRAMPOLINE_LEN];
    let page_ptr: *mut c_void = alloc_nearby_rx_page(callee_addr, &placeholder);
    let tramp_addr: usize = page_ptr as usize;
    let stub: [u8; ON_DESTROY_CLEAR_STARTED_TRAMPOLINE_LEN] =
        encode_on_destroy_clear_started_trampoline(tramp_addr, callee_addr)
            .expect("encode on_destroy_clear_started trampoline");

    let mut old_prot: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Writing real rel32 stub into our private test page before restoring `RX`.
    unsafe {
        VirtualProtect(page_ptr, 0x1000, PAGE_READWRITE, &raw mut old_prot).unwrap();
        std::ptr::copy_nonoverlapping(stub.as_ptr(), page_ptr.cast::<u8>(), stub.len());
        VirtualProtect(page_ptr, 0x1000, PAGE_EXECUTE_READ, &raw mut old_prot).unwrap();
        FlushInstructionCache(GetCurrentProcess(), Some(page_ptr), 0x1000).unwrap();
    }

    let invoke = |session_ptr: usize| -> usize {
        let ret_val: usize;
        // SAFETY: Invokes our 17-byte `RX` trampoline with `rcx = session_ptr`, matching `0xE7EDC9`.
        unsafe {
            core::arch::asm!(
                "sub rsp, 0x28",
                "call r10",
                "add rsp, 0x28",
                inout("r10") tramp_addr => _,
                inout("rcx") session_ptr => _,
                lateout("rax") ret_val,
                clobber_abi("C"),
            );
        }
        ret_val
    };

    CALL_COUNT.store(0, Ordering::SeqCst);
    let mut session: MockOnlineSession = MockOnlineSession {
        pad: [0x77u8; 0x16E],
        b_has_game_started: 1,
        sentinel_after: 0xAA,
    };
    let session_ptr: usize = (&raw mut session) as usize;
    assert_eq!(invoke(session_ptr), 0xDEAD_1234);
    assert_eq!(session.b_has_game_started, 0);
    assert_eq!(session.sentinel_after, 0xAA);
    assert!(session.pad.iter().all(|b: &u8| *b == 0x77u8));
    assert_eq!(CAPTURED_CTX.load(Ordering::SeqCst), session_ptr);
    assert_eq!(CAPTURED_FLAG_AT_CALLEE.load(Ordering::SeqCst), 0);

    // NULL rcx safety check -> forwards to callee without dereferencing.
    assert_eq!(invoke(0), 0);
    assert_eq!(CAPTURED_CTX.load(Ordering::SeqCst), 0);
    assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 2);

    // SAFETY: Release test page.
    unsafe {
        VirtualFree(page_ptr, 0, MEM_RELEASE).expect("VirtualFree");
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn on_join_reset_uri_trampoline_resets_uri_then_tail_calls_update() {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
    use windows::Win32::System::Memory::{
        VirtualFree, VirtualProtect, MEM_RELEASE, PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS,
        PAGE_READWRITE,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    static NEXT_GI_PTR: AtomicUsize = AtomicUsize::new(0);
    static CALL_TRACE: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_GI_CTX: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_RESET_GI: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_UPDATE_THIS: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn mock_get_gi(ctx: usize) -> usize {
        CAPTURED_GI_CTX.store(ctx, Ordering::SeqCst);
        let prev: usize = CALL_TRACE.load(Ordering::SeqCst);
        CALL_TRACE.store(prev * 10 + 1, Ordering::SeqCst);
        NEXT_GI_PTR.load(Ordering::SeqCst)
    }

    extern "C" fn mock_reset_uri(gi: usize) {
        CAPTURED_RESET_GI.store(gi, Ordering::SeqCst);
        let prev: usize = CALL_TRACE.load(Ordering::SeqCst);
        CALL_TRACE.store(prev * 10 + 2, Ordering::SeqCst);
    }

    extern "C" fn mock_update(this_ptr: usize) -> usize {
        CAPTURED_UPDATE_THIS.store(this_ptr, Ordering::SeqCst);
        let prev: usize = CALL_TRACE.load(Ordering::SeqCst);
        CALL_TRACE.store(prev * 10 + 3, Ordering::SeqCst);
        0x55AAusize
    }

    let gi_addr: usize = mock_get_gi as *const () as usize;
    let reset_addr: usize = mock_reset_uri as *const () as usize;
    let update_addr: usize = mock_update as *const () as usize;

    let placeholder: [u8; ON_JOIN_RESET_URI_TRAMPOLINE_LEN] =
        [0xCCu8; ON_JOIN_RESET_URI_TRAMPOLINE_LEN];
    let page_ptr: *mut c_void = alloc_nearby_rx_page(gi_addr, &placeholder);
    let tramp_addr: usize = page_ptr as usize;
    let stub: [u8; ON_JOIN_RESET_URI_TRAMPOLINE_LEN] =
        encode_on_join_reset_uri_trampoline(tramp_addr, gi_addr, reset_addr, update_addr)
            .expect("encode on_join_reset_uri trampoline");

    let mut old_prot: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Writing real rel32 stub into our private test page before restoring `RX`.
    unsafe {
        VirtualProtect(page_ptr, 0x1000, PAGE_READWRITE, &raw mut old_prot).unwrap();
        std::ptr::copy_nonoverlapping(stub.as_ptr(), page_ptr.cast::<u8>(), stub.len());
        VirtualProtect(page_ptr, 0x1000, PAGE_EXECUTE_READ, &raw mut old_prot).unwrap();
        FlushInstructionCache(GetCurrentProcess(), Some(page_ptr), 0x1000).unwrap();
    }

    let invoke = |session_this: usize, gi_ret: usize| -> usize {
        NEXT_GI_PTR.store(gi_ret, Ordering::SeqCst);
        CALL_TRACE.store(0, Ordering::SeqCst);
        CAPTURED_GI_CTX.store(0, Ordering::SeqCst);
        CAPTURED_RESET_GI.store(0, Ordering::SeqCst);
        CAPTURED_UPDATE_THIS.store(0, Ordering::SeqCst);
        let ret_val: usize;
        // SAFETY: Invokes our 41-byte `RX` trampoline with `rcx = session_this`, matching `0xE815D7`.
        unsafe {
            core::arch::asm!(
                "sub rsp, 0x28",
                "call r10",
                "add rsp, 0x28",
                inout("r10") tramp_addr => _,
                inout("rcx") session_this => _,
                lateout("rax") ret_val,
                clobber_abi("C"),
            );
        }
        ret_val
    };

    // Case 1: Valid GameInstance -> executes GetInstance(this) -> ResetURI(gi) -> Update(this).
    assert_eq!(invoke(0x1122_3344, 0x9988_7766), 0x55AA);
    assert_eq!(CALL_TRACE.load(Ordering::SeqCst), 123);
    assert_eq!(CAPTURED_GI_CTX.load(Ordering::SeqCst), 0x1122_3344);
    assert_eq!(CAPTURED_RESET_GI.load(Ordering::SeqCst), 0x9988_7766);
    assert_eq!(CAPTURED_UPDATE_THIS.load(Ordering::SeqCst), 0x1122_3344);

    // Case 2: Null GameInstance -> skips ResetURI safely and still tail-calls Update(this).
    assert_eq!(invoke(0x5566_7788, 0), 0x55AA);
    assert_eq!(CALL_TRACE.load(Ordering::SeqCst), 13);
    assert_eq!(CAPTURED_GI_CTX.load(Ordering::SeqCst), 0x5566_7788);
    assert_eq!(CAPTURED_RESET_GI.load(Ordering::SeqCst), 0);
    assert_eq!(CAPTURED_UPDATE_THIS.load(Ordering::SeqCst), 0x5566_7788);

    // SAFETY: Release test page.
    unsafe {
        VirtualFree(page_ptr, 0, MEM_RELEASE).expect("VirtualFree");
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn on_participants_keep_members_trampoline_guards_started_matches_only() {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
    use windows::Win32::System::Memory::{
        VirtualFree, VirtualProtect, MEM_RELEASE, PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS,
        PAGE_READWRITE,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    #[repr(C)]
    struct MockOnlineSessionTime {
        pad: [u8; 0x48],
        start_utc_date_time: i64,
    }

    static IS_LOBBY_MAP_RET: AtomicBool = AtomicBool::new(false);
    static IS_LOBBY_MAP_CALLS: AtomicUsize = AtomicUsize::new(0);
    static CAPTURED_CTX: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn mock_is_lobby_map(ctx: usize) -> u8 {
        CAPTURED_CTX.store(ctx, Ordering::SeqCst);
        IS_LOBBY_MAP_CALLS.fetch_add(1, Ordering::SeqCst);
        u8::from(IS_LOBBY_MAP_RET.load(Ordering::SeqCst))
    }

    let is_lobby_addr: usize = mock_is_lobby_map as *const () as usize;
    let placeholder: [u8; ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE_LEN] =
        [0xCCu8; ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE_LEN];
    let page_ptr: *mut c_void = alloc_nearby_rx_page(is_lobby_addr, &placeholder);
    let tramp_addr: usize = page_ptr as usize;
    let stub: [u8; ON_PARTICIPANTS_KEEP_MEMBERS_TRAMPOLINE_LEN] =
        encode_on_participants_keep_members_trampoline(tramp_addr, is_lobby_addr)
            .expect("encode on_participants_keep_members trampoline");

    let mut old_prot: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS(0);
    // SAFETY: Writing real rel32 stub into our private test page before restoring `RX`.
    unsafe {
        VirtualProtect(page_ptr, 0x1000, PAGE_READWRITE, &raw mut old_prot).unwrap();
        std::ptr::copy_nonoverlapping(stub.as_ptr(), page_ptr.cast::<u8>(), stub.len());
        VirtualProtect(page_ptr, 0x1000, PAGE_EXECUTE_READ, &raw mut old_prot).unwrap();
        FlushInstructionCache(GetCurrentProcess(), Some(page_ptr), 0x1000).unwrap();
    }

    let invoke = |session_ptr: usize, lobby_ret: bool| -> u8 {
        IS_LOBBY_MAP_RET.store(lobby_ret, Ordering::SeqCst);
        let ret_val: usize;
        // SAFETY: Invokes our 20-byte `RX` trampoline with `rcx = session_ptr`, matching `0xE85B28`.
        unsafe {
            core::arch::asm!(
                "sub rsp, 0x28",
                "call r10",
                "add rsp, 0x28",
                inout("r10") tramp_addr => _,
                inout("rcx") session_ptr => _,
                lateout("rax") ret_val,
                clobber_abi("C"),
            );
        }
        u8::try_from(ret_val & 0xFF).unwrap()
    };

    IS_LOBBY_MAP_CALLS.store(0, Ordering::SeqCst);

    // Case 1: Match already started (StartUTCDateTime != 0) -> returns 0 WITHOUT calling IsLobbyMap!
    let started: MockOnlineSessionTime = MockOnlineSessionTime {
        pad: [0u8; 0x48],
        start_utc_date_time: 1_795_564_800,
    };
    assert_eq!(invoke((&raw const started) as usize, true), 0);
    assert_eq!(IS_LOBBY_MAP_CALLS.load(Ordering::SeqCst), 0);

    // Case 2: Pre-match lobby (StartUTCDateTime == 0) -> delegates to IsLobbyMap and returns 1.
    let pre_match: MockOnlineSessionTime = MockOnlineSessionTime {
        pad: [0u8; 0x48],
        start_utc_date_time: 0,
    };
    let pre_ptr: usize = (&raw const pre_match) as usize;
    assert_eq!(invoke(pre_ptr, true), 1);
    assert_eq!(IS_LOBBY_MAP_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(CAPTURED_CTX.load(Ordering::SeqCst), pre_ptr);

    // Case 3: Pre-match non-lobby -> delegates to IsLobbyMap and returns 0.
    assert_eq!(invoke(pre_ptr, false), 0);
    assert_eq!(IS_LOBBY_MAP_CALLS.load(Ordering::SeqCst), 2);

    // Case 4: Null rcx -> safely delegates to IsLobbyMap(0).
    assert_eq!(invoke(0, true), 1);
    assert_eq!(IS_LOBBY_MAP_CALLS.load(Ordering::SeqCst), 3);
    assert_eq!(CAPTURED_CTX.load(Ordering::SeqCst), 0);

    // SAFETY: Release test page.
    unsafe {
        VirtualFree(page_ptr, 0, MEM_RELEASE).expect("VirtualFree");
    }
}
