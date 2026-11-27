use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use dh_voice_client::consts::patch::{
    DETOUR_GET_WANTS_TO_TALK, DETOUR_IS_PARTICIPANT_SPEAKING, DETOUR_IS_PLAYER_IN_VOICE_CHAT,
    DETOUR_LOAD_CHANNEL_DEFINITIONS, DETOUR_ONLINE_VOICE_TICK, DETOUR_SET_WANTS_TO_TALK,
    DIRECT_PATCHES, EOS_BEFORE_RECV_AUDIO_GUARD, EOS_PLUS_GET_VOICE_VTABLE_PATCH,
    EOS_RECV_GUARD_STUB_LEN, SUBMIT_REMOTE_VOICE_HOOK, SUBMIT_REMOTE_VOICE_STUB_LEN,
    X64_ABS_JMP_LEN,
};
use dh_voice_client::consts::proxy::{X3DAUDIO_EXPORTS, X3DAUDIO_EXPORT_COUNT};
use dh_voice_client::{
    apply_direct_patch_in_buffer, encode_abs_jmp_stub, encode_eos_recv_guard_stub,
    encode_jmp_rel32, encode_original_trampoline, encode_submit_remote_voice_stub,
    is_target_process_path, ChannelDefinition, DetourSpec, FName, FString, PatchOutcome, SharedPtr,
    TArrayRaw, TArrayViewI16, TrampolineError,
};

fn rva_to_file_offset(pe_bytes: &[u8], rva: usize) -> Option<usize> {
    let pe_off: usize = usize::try_from(u32::from_le_bytes(
        pe_bytes.get(0x3C..0x40)?.try_into().ok()?,
    ))
    .ok()?;
    let section_count: usize = usize::from(u16::from_le_bytes(
        pe_bytes.get(pe_off + 6..pe_off + 8)?.try_into().ok()?,
    ));
    let opt_size: usize = usize::from(u16::from_le_bytes(
        pe_bytes.get(pe_off + 20..pe_off + 22)?.try_into().ok()?,
    ));
    let sec_base: usize = pe_off + 24 + opt_size;
    for idx in 0..section_count {
        let entry: usize = sec_base + idx * 40;
        let virt_size: usize = usize::try_from(u32::from_le_bytes(
            pe_bytes.get(entry + 8..entry + 12)?.try_into().ok()?,
        ))
        .ok()?;
        let virt_addr: usize = usize::try_from(u32::from_le_bytes(
            pe_bytes.get(entry + 12..entry + 16)?.try_into().ok()?,
        ))
        .ok()?;
        let raw_size: usize = usize::try_from(u32::from_le_bytes(
            pe_bytes.get(entry + 16..entry + 20)?.try_into().ok()?,
        ))
        .ok()?;
        let raw_ptr: usize = usize::try_from(u32::from_le_bytes(
            pe_bytes.get(entry + 20..entry + 24)?.try_into().ok()?,
        ))
        .ok()?;
        let span: usize = virt_size.max(raw_size);
        if rva >= virt_addr && rva < virt_addr.checked_add(span)? {
            return raw_ptr.checked_add(rva - virt_addr);
        }
    }
    None
}

#[test]
fn struct_layouts_match_ue4_abi() {
    assert_eq!(std::mem::size_of::<AtomicBool>(), 1);
    assert_eq!(std::mem::size_of::<FName>(), 0x08);
    assert_eq!(std::mem::size_of::<FString>(), 0x10);
    assert_eq!(std::mem::size_of::<TArrayRaw>(), 0x10);
    assert_eq!(std::mem::size_of::<TArrayViewI16>(), 0x10);
    assert_eq!(std::mem::size_of::<SharedPtr>(), 0x10);
    assert_eq!(std::mem::size_of::<ChannelDefinition>(), 0x28);
}

#[test]
fn target_process_path_guard_matches_client_only() {
    assert!(is_target_process_path("DreadHunger-Win64-Shipping.exe"));
    assert!(is_target_process_path(
        r"DreadHunger\Binaries\Win64\dreadhunger-win64-shipping.exe"
    ));
    assert!(!is_target_process_path(
        "DreadHungerServer-Win64-Shipping.exe"
    ));
    assert!(!is_target_process_path("DreadHunger.exe"));
}

#[test]
fn x3daudio_exports_cover_ordinals_one_and_two() {
    assert_eq!(X3DAUDIO_EXPORTS.len(), X3DAUDIO_EXPORT_COUNT);
    assert_eq!(X3DAUDIO_EXPORTS[0].ordinal, 1);
    assert_eq!(X3DAUDIO_EXPORTS[0].name, "X3DAudioCalculate");
    assert_eq!(X3DAUDIO_EXPORTS[1].ordinal, 2);
    assert_eq!(X3DAUDIO_EXPORTS[1].name, "X3DAudioInitialize");
}

#[test]
fn direct_patch_in_buffer_is_exact_and_idempotent() {
    let mut buf: Vec<u8> = vec![0u8; 32];
    let patch = dh_voice_client::BytePatch {
        name: "synthetic",
        rva: 8,
        expected: &[0x48, 0x8B, 0x01],
        replacement: &[0xC3, 0x90, 0x90],
    };
    buf[8..11].copy_from_slice(patch.expected);

    assert_eq!(
        apply_direct_patch_in_buffer(&mut buf, &patch),
        PatchOutcome::Applied
    );
    assert_eq!(&buf[8..11], patch.replacement);
    assert_eq!(
        apply_direct_patch_in_buffer(&mut buf, &patch),
        PatchOutcome::AlreadyApplied
    );

    buf[8] ^= 0xFF;
    assert_eq!(
        apply_direct_patch_in_buffer(&mut buf, &patch),
        PatchOutcome::SignatureMismatch
    );
}

#[test]
fn custom_stubs_encode_exact_x64_sequences() {
    let submit_stub: [u8; SUBMIT_REMOTE_VOICE_STUB_LEN] =
        encode_submit_remote_voice_stub(0x0000_7FF8_AABB_CCDD, 0x0000_0001_4092_824B).unwrap();
    assert_eq!(&submit_stub[0..4], &[0x48, 0x83, 0xEC, 0x20]);
    assert_eq!(&submit_stub[24..26], &[0x48, 0xB8]);
    assert_eq!(
        u64::from_le_bytes(submit_stub[26..34].try_into().unwrap()),
        0x0000_7FF8_AABB_CCDD
    );
    assert_eq!(&submit_stub[34..40], &[0xFF, 0xD0, 0x48, 0x83, 0xC4, 0x20]);
    assert_eq!(&submit_stub[40..46], &[0xFF, 0x25, 0x00, 0x00, 0x00, 0x00]);
    assert_eq!(
        u64::from_le_bytes(submit_stub[46..54].try_into().unwrap()),
        0x0000_0001_4092_824B
    );

    let guard_stub: [u8; EOS_RECV_GUARD_STUB_LEN] = encode_eos_recv_guard_stub(
        0x0000_7FF8_1122_3344,
        &EOS_BEFORE_RECV_AUDIO_GUARD.expected_prologue,
        0x0000_0001_40A2_DC7A,
    )
    .unwrap();
    assert_eq!(&guard_stub[0..2], &[0x48, 0xB8]);
    assert_eq!(
        u64::from_le_bytes(guard_stub[2..10].try_into().unwrap()),
        0x0000_7FF8_1122_3344
    );
    assert_eq!(&guard_stub[10..16], &[0x80, 0x38, 0x00, 0x74, 0x01, 0xC3]);
    assert_eq!(
        &guard_stub[16..26],
        &EOS_BEFORE_RECV_AUDIO_GUARD.expected_prologue
    );
    assert_eq!(&guard_stub[26..32], &[0xFF, 0x25, 0x00, 0x00, 0x00, 0x00]);
    assert_eq!(
        u64::from_le_bytes(guard_stub[32..40].try_into().unwrap()),
        0x0000_0001_40A2_DC7A
    );

    let orig: Vec<u8> =
        encode_original_trampoline(&[0x40, 0x53, 0x48, 0x83, 0xEC, 0x30], 0x1409_29696).unwrap();
    assert_eq!(orig.len(), 6 + X64_ABS_JMP_LEN);

    let abs: [u8; X64_ABS_JMP_LEN] = encode_abs_jmp_stub(0x1400_1234).unwrap();
    assert_eq!(
        u64::from_le_bytes(abs[6..14].try_into().unwrap()),
        0x1400_1234
    );

    assert_eq!(
        encode_jmp_rel32(0x1000, 0x1_0000_0000),
        Err(TrampolineError::DisplacementOutOfRange)
    );
}

#[test]
fn client_binary_signatures_match_reference_executable() {
    let client_exe: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../references/WindowsClient/DreadHunger-Win64-Shipping.exe");
    if !client_exe.is_file() {
        return;
    }

    let bytes: Vec<u8> = fs::read(&client_exe).expect("read reference client exe");

    // 1. Direct byte patches (7)
    for patch in &DIRECT_PATCHES {
        let off: usize =
            rva_to_file_offset(&bytes, patch.rva).expect("client direct patch RVA maps to file");
        let actual: &[u8] = &bytes[off..off + patch.expected.len()];
        assert_eq!(
            actual, patch.expected,
            "byte mismatch for client patch '{}' at RVA 0x{:X}",
            patch.name, patch.rva
        );
    }

    // 2. Vtable slot (`FOnlineSubsystemEOSPlus::GetVoiceInterface` at default ImageBase `0x140000000`)
    let vt_off: usize = rva_to_file_offset(&bytes, EOS_PLUS_GET_VOICE_VTABLE_PATCH.slot_rva)
        .expect("EOSPlus vtable slot RVA maps to file");
    let stored_va: u64 = u64::from_le_bytes(bytes[vt_off..vt_off + 8].try_into().unwrap());
    let expected_va: u64 = 0x0000_0001_4000_0000u64
        + u64::try_from(EOS_PLUS_GET_VOICE_VTABLE_PATCH.expected_target_rva).unwrap();
    assert_eq!(
        stored_va, expected_va,
        "vtable mismatch for '{}'",
        EOS_PLUS_GET_VOICE_VTABLE_PATCH.name
    );

    // 3. Mid-function hook (`SubmitRemoteVoiceData`)
    let submit_off: usize = rva_to_file_offset(&bytes, SUBMIT_REMOTE_VOICE_HOOK.hook_rva)
        .expect("SubmitRemoteVoiceData hook RVA maps to file");
    assert_eq!(
        &bytes[submit_off..submit_off + SUBMIT_REMOTE_VOICE_HOOK.expected_bytes.len()],
        SUBMIT_REMOTE_VOICE_HOOK.expected_bytes,
        "byte mismatch for '{}'",
        SUBMIT_REMOTE_VOICE_HOOK.name
    );

    // 4. Guard detour (`OnVoiceChatBeforeRecvAudioRendered`)
    let guard_off: usize = rva_to_file_offset(&bytes, EOS_BEFORE_RECV_AUDIO_GUARD.rva)
        .expect("EOS recv guard RVA maps to file");
    assert_eq!(
        &bytes[guard_off..guard_off + EOS_BEFORE_RECV_AUDIO_GUARD.expected_prologue.len()],
        &EOS_BEFORE_RECV_AUDIO_GUARD.expected_prologue,
        "prologue mismatch for '{}'",
        EOS_BEFORE_RECV_AUDIO_GUARD.name
    );

    // 5. Standard function detours (6)
    let detours: [&DetourSpec; 6] = [
        &DETOUR_LOAD_CHANNEL_DEFINITIONS,
        &DETOUR_ONLINE_VOICE_TICK,
        &DETOUR_SET_WANTS_TO_TALK,
        &DETOUR_GET_WANTS_TO_TALK,
        &DETOUR_IS_PARTICIPANT_SPEAKING,
        &DETOUR_IS_PLAYER_IN_VOICE_CHAT,
    ];
    for spec in detours {
        let off: usize =
            rva_to_file_offset(&bytes, spec.rva).expect("client detour RVA maps to file");
        let actual: &[u8] = &bytes[off..off + spec.expected_prologue.len()];
        assert_eq!(
            actual, spec.expected_prologue,
            "prologue mismatch for client detour '{}' at RVA 0x{:X}",
            spec.name, spec.rva
        );
    }
}
