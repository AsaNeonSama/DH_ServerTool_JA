use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use dh_voice_client::consts::opus::{
    RVA_OPUS_DECODE, RVA_OPUS_DECODER_CTL, RVA_OPUS_DECODER_GET_SIZE, RVA_OPUS_DECODER_INIT,
    RVA_OPUS_ENCODE, RVA_OPUS_ENCODER_CTL, RVA_OPUS_ENCODER_GET_SIZE, RVA_OPUS_ENCODER_INIT,
    RVA_OPUS_GET_VERSION_STRING, SIG_OPUS_DECODE, SIG_OPUS_DECODER_CTL, SIG_OPUS_DECODER_GET_SIZE,
    SIG_OPUS_DECODER_INIT, SIG_OPUS_ENCODE, SIG_OPUS_ENCODER_CTL, SIG_OPUS_ENCODER_GET_SIZE,
    SIG_OPUS_ENCODER_INIT, SIG_OPUS_GET_VERSION_STRING,
};
use dh_voice_client::consts::patch::{
    DETOUR_ENGINE_LOOP_TICK, DETOUR_GET_PARTICIPANT_AUDIO_ENERGY, DETOUR_GET_WANTS_TO_TALK,
    DETOUR_IS_PARTICIPANT_SPEAKING, DETOUR_IS_PLAYER_IN_VOICE_CHAT, DETOUR_SET_WANTS_TO_TALK,
    DETOUR_SOCKET_CLOSE, DETOUR_SOCKET_RECV_FROM, DETOUR_SOCKET_SEND_TO,
    EOS_BEFORE_RECV_AUDIO_GUARD, EOS_RECV_GUARD_STUB_LEN, STANDARD_DETOUR_COUNT, TOTAL_PATCH_COUNT,
    X64_ABS_JMP_LEN,
};
use dh_voice_client::consts::proxy::{X3DAUDIO_EXPORTS, X3DAUDIO_EXPORT_COUNT};
use dh_voice_client::consts::ue4::{
    RVA_PROCESS_MULTICAST_DELEGATE, RVA_UIPNETDRIVER_GET_SOCKET, SIG_PROCESS_MULTICAST_DELEGATE,
};
use dh_voice_client::{
    encode_abs_jmp_stub, encode_eos_recv_guard_stub, encode_jmp_rel32, encode_original_trampoline,
    is_target_process_path, DetourSpec, FString, TArrayViewI16, TrampolineError,
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
    assert_eq!(std::mem::size_of::<FString>(), 0x10);
    assert_eq!(std::mem::size_of::<TArrayViewI16>(), 0x10);
    assert_eq!(TOTAL_PATCH_COUNT, 1 + STANDARD_DETOUR_COUNT);
    assert_eq!(TOTAL_PATCH_COUNT, 10);
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
fn custom_stubs_encode_exact_x64_sequences() {
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

    let orig_close: Vec<u8> =
        encode_original_trampoline(DETOUR_SOCKET_CLOSE.expected_prologue, 0x0000_0001_4166_0B16)
            .unwrap();
    assert_eq!(orig_close.len(), 6 + X64_ABS_JMP_LEN);

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
fn client_binary_signatures_and_libopus_match_reference_executable() {
    let client_exe: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../references/WindowsClient/DreadHunger-Win64-Shipping.exe");
    if !client_exe.is_file() {
        return;
    }

    let bytes: Vec<u8> = fs::read(&client_exe).expect("read reference client exe");

    // 1. Guard detour (`FEOSPlusVoiceChatUser::OnVoiceChatBeforeRecvAudioRendered`)
    let guard_off: usize = rva_to_file_offset(&bytes, EOS_BEFORE_RECV_AUDIO_GUARD.rva)
        .expect("EOS recv guard RVA maps to file");
    assert_eq!(
        &bytes[guard_off..guard_off + EOS_BEFORE_RECV_AUDIO_GUARD.expected_prologue.len()],
        &EOS_BEFORE_RECV_AUDIO_GUARD.expected_prologue,
        "prologue mismatch for '{}'",
        EOS_BEFORE_RECV_AUDIO_GUARD.name
    );

    // 2. All 9 Plan A standard function detours
    let detours: [&DetourSpec; STANDARD_DETOUR_COUNT] = [
        &DETOUR_SOCKET_RECV_FROM,
        &DETOUR_SOCKET_SEND_TO,
        &DETOUR_SOCKET_CLOSE,
        &DETOUR_ENGINE_LOOP_TICK,
        &DETOUR_SET_WANTS_TO_TALK,
        &DETOUR_GET_WANTS_TO_TALK,
        &DETOUR_IS_PARTICIPANT_SPEAKING,
        &DETOUR_IS_PLAYER_IN_VOICE_CHAT,
        &DETOUR_GET_PARTICIPANT_AUDIO_ENERGY,
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

    // 3. `UIpNetDriver::GetSocket` and `ProcessMulticastDelegate` entry prologue verification
    let get_sock_off: usize = rva_to_file_offset(&bytes, RVA_UIPNETDRIVER_GET_SOCKET)
        .expect("UIpNetDriver::GetSocket RVA maps to file");
    assert_eq!(
        &bytes[get_sock_off..get_sock_off + 5],
        &[0x48, 0x89, 0x5C, 0x24, 0x08],
        "UIpNetDriver::GetSocket prologue mismatch"
    );

    let multicast_off: usize = rva_to_file_offset(&bytes, RVA_PROCESS_MULTICAST_DELEGATE)
        .expect("ProcessMulticastDelegate RVA maps to file");
    assert_eq!(
        &bytes[multicast_off..multicast_off + SIG_PROCESS_MULTICAST_DELEGATE.len()],
        &SIG_PROCESS_MULTICAST_DELEGATE,
        "ProcessMulticastDelegate prologue mismatch at RVA 0x{RVA_PROCESS_MULTICAST_DELEGATE:X}"
    );

    // 4. All 9 in-process `libopus 1.1-beta` function signatures
    let opus_entries: [(&str, usize, [u8; 8]); 9] = [
        (
            "opus_encoder_get_size",
            RVA_OPUS_ENCODER_GET_SIZE,
            SIG_OPUS_ENCODER_GET_SIZE,
        ),
        (
            "opus_encoder_init",
            RVA_OPUS_ENCODER_INIT,
            SIG_OPUS_ENCODER_INIT,
        ),
        (
            "opus_encoder_ctl",
            RVA_OPUS_ENCODER_CTL,
            SIG_OPUS_ENCODER_CTL,
        ),
        ("opus_encode", RVA_OPUS_ENCODE, SIG_OPUS_ENCODE),
        (
            "opus_decoder_get_size",
            RVA_OPUS_DECODER_GET_SIZE,
            SIG_OPUS_DECODER_GET_SIZE,
        ),
        (
            "opus_decoder_init",
            RVA_OPUS_DECODER_INIT,
            SIG_OPUS_DECODER_INIT,
        ),
        (
            "opus_decoder_ctl",
            RVA_OPUS_DECODER_CTL,
            SIG_OPUS_DECODER_CTL,
        ),
        ("opus_decode", RVA_OPUS_DECODE, SIG_OPUS_DECODE),
        (
            "opus_get_version_string",
            RVA_OPUS_GET_VERSION_STRING,
            SIG_OPUS_GET_VERSION_STRING,
        ),
    ];
    for (name, rva, expected) in opus_entries {
        let off: usize = rva_to_file_offset(&bytes, rva).expect("libopus RVA maps to file");
        let actual: &[u8] = &bytes[off..off + expected.len()];
        assert_eq!(
            actual, &expected,
            "libopus signature mismatch for '{name}' at RVA 0x{rva:X}"
        );
    }
}
