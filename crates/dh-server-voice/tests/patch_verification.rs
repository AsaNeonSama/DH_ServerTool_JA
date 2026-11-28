//! Verification tests for `dh-server-voice` detour encoding, PE parsing, and reference binary prologues.

use std::path::PathBuf;

use dh_voice_server::consts::patch::{
    EXPECTED_CLOSE_PROLOGUE, EXPECTED_RECEIVED_PACKET_PROLOGUE, EXPECTED_RECV_FROM_PROLOGUE,
    RVA_FINTERNET_ADDR_BSD_VTABLE, RVA_FSOCKET_BSD_CLOSE, RVA_FSOCKET_BSD_RECV_FROM,
    RVA_FSOCKET_BSD_VTABLE, RVA_UIP_CONNECTION_VTABLE, RVA_UNET_CONNECTION_EOS_VTABLE,
    RVA_UNET_CONNECTION_RECEIVED_PACKET, RVA_USTEAM_NET_CONNECTION_VTABLE,
    X64_JMP_ABS_INDIRECT_PREFIX, X64_JMP_REL32_OPCODE, X64_NOP_BYTE,
};
use dh_voice_server::{
    align_down, align_up, encode_detour_site_patch, encode_jmp_abs_indirect, encode_jmp_rel32,
    encode_original_trampoline, is_target_process_path, DetourError,
};

#[test]
fn target_process_name_matching_is_case_insensitive() {
    assert!(is_target_process_path(
        "DreadHungerServer-Win64-Shipping.exe"
    ));
    assert!(is_target_process_path(
        "DreadHunger\\Binaries\\Win64\\dreadhungerserver-win64-shipping.exe\0"
    ));
    assert!(!is_target_process_path("DreadHunger-Win64-Shipping.exe"));
    assert!(!is_target_process_path("notepad.exe"));
}

#[test]
fn alignment_and_x64_branch_encoders() {
    assert_eq!(align_up(0x1001, 0x1000), Some(0x2000));
    assert_eq!(align_up(0x2000, 0x1000), Some(0x2000));
    assert_eq!(align_up(0x1000, 0), None);
    assert_eq!(align_down(0x1FFF, 0x1000), Some(0x1000));
    assert_eq!(align_down(0x1000, 0), None);

    // Forward jmp rel32: 0x1_4000_1000 -> 0x1_4000_2005 => disp = +0x1000
    let jmp: [u8; 5] = encode_jmp_rel32(0x1_4000_1000, 0x1_4000_2005).unwrap();
    assert_eq!(jmp[0], X64_JMP_REL32_OPCODE);
    assert_eq!(i32::from_le_bytes([jmp[1], jmp[2], jmp[3], jmp[4]]), 0x1000);

    // Out-of-range (> 2 GiB) must return DisplacementOutOfRange.
    assert_eq!(
        encode_jmp_rel32(0x1_0000_0000, 0x2_0000_0000),
        Err(DetourError::DisplacementOutOfRange)
    );

    // 6-byte site patch (Close / ReceivedPacket): 5-byte jmp rel32 + 1 NOP.
    let close_patch: Vec<u8> =
        encode_detour_site_patch(0x1_4158_49C0, 0x1_4500_0040, EXPECTED_CLOSE_PROLOGUE.len())
            .unwrap();
    assert_eq!(close_patch.len(), 6);
    assert_eq!(close_patch[0], X64_JMP_REL32_OPCODE);
    assert_eq!(close_patch[5], X64_NOP_BYTE);

    let recv_pkt_patch: Vec<u8> = encode_detour_site_patch(
        0x1_4268_2090,
        0x1_4500_0080,
        EXPECTED_RECEIVED_PACKET_PROLOGUE.len(),
    )
    .unwrap();
    assert_eq!(recv_pkt_patch.len(), 6);
    assert_eq!(recv_pkt_patch[0], X64_JMP_REL32_OPCODE);
    assert_eq!(recv_pkt_patch[5], X64_NOP_BYTE);

    // Absolute indirect jump encoding: FF 25 00 00 00 00 <u64 LE>.
    let abs_target: usize = 0x7FF8_1234_5678;
    let abs_jmp: [u8; 14] = encode_jmp_abs_indirect(abs_target).unwrap();
    assert_eq!(&abs_jmp[..6], &X64_JMP_ABS_INDIRECT_PREFIX);
    assert_eq!(
        u64::from_le_bytes([
            abs_jmp[6],
            abs_jmp[7],
            abs_jmp[8],
            abs_jmp[9],
            abs_jmp[10],
            abs_jmp[11],
            abs_jmp[12],
            abs_jmp[13]
        ]),
        abs_target as u64
    );

    // Original trampoline = stolen prologue + 14-byte abs jump to resume_addr.
    let tramp: Vec<u8> =
        encode_original_trampoline(&EXPECTED_RECV_FROM_PROLOGUE, 0x1_4158_A1F5).unwrap();
    assert_eq!(tramp.len(), EXPECTED_RECV_FROM_PROLOGUE.len() + 14);
    assert_eq!(&tramp[..5], &EXPECTED_RECV_FROM_PROLOGUE);
    assert_eq!(&tramp[5..11], &X64_JMP_ABS_INDIRECT_PREFIX);
}

fn rva_to_file_offset(pe: &[u8], rva: usize) -> Option<usize> {
    let lfanew: usize =
        usize::try_from(u32::from_le_bytes([pe[0x3C], pe[0x3D], pe[0x3E], pe[0x3F]])).ok()?;
    let num_sections: usize = usize::from(u16::from_le_bytes([pe[lfanew + 6], pe[lfanew + 7]]));
    let opt_hdr_size: usize = usize::from(u16::from_le_bytes([pe[lfanew + 20], pe[lfanew + 21]]));
    let sec_table: usize = lfanew + 24 + opt_hdr_size;

    for idx in 0..num_sections {
        let off: usize = sec_table + idx * 40;
        let virt_size: usize = usize::try_from(u32::from_le_bytes([
            pe[off + 8],
            pe[off + 9],
            pe[off + 10],
            pe[off + 11],
        ]))
        .ok()?;
        let virt_addr: usize = usize::try_from(u32::from_le_bytes([
            pe[off + 12],
            pe[off + 13],
            pe[off + 14],
            pe[off + 15],
        ]))
        .ok()?;
        let raw_size: usize = usize::try_from(u32::from_le_bytes([
            pe[off + 16],
            pe[off + 17],
            pe[off + 18],
            pe[off + 19],
        ]))
        .ok()?;
        let raw_ptr: usize = usize::try_from(u32::from_le_bytes([
            pe[off + 20],
            pe[off + 21],
            pe[off + 22],
            pe[off + 23],
        ]))
        .ok()?;
        let span: usize = virt_size.max(raw_size);
        if rva >= virt_addr && rva < virt_addr + span {
            return Some(raw_ptr + (rva - virt_addr));
        }
    }
    None
}

#[test]
fn reference_server_binary_prologues_and_vtables_match_if_present() {
    let manifest_dir: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ref_exe: PathBuf = manifest_dir
        .join("..")
        .join("..")
        .join("..")
        .join("references")
        .join("WindowsServer")
        .join("DreadHunger")
        .join("Binaries")
        .join("Win64")
        .join("DreadHungerServer-Win64-Shipping.exe");

    if !ref_exe.is_file() {
        return;
    }

    let pe_bytes: Vec<u8> = std::fs::read(&ref_exe).unwrap();

    let recv_off: usize = rva_to_file_offset(&pe_bytes, RVA_FSOCKET_BSD_RECV_FROM).unwrap();
    assert_eq!(
        &pe_bytes[recv_off..recv_off + EXPECTED_RECV_FROM_PROLOGUE.len()],
        &EXPECTED_RECV_FROM_PROLOGUE
    );

    let close_off: usize = rva_to_file_offset(&pe_bytes, RVA_FSOCKET_BSD_CLOSE).unwrap();
    assert_eq!(
        &pe_bytes[close_off..close_off + EXPECTED_CLOSE_PROLOGUE.len()],
        &EXPECTED_CLOSE_PROLOGUE
    );

    let recv_pkt_off: usize =
        rva_to_file_offset(&pe_bytes, RVA_UNET_CONNECTION_RECEIVED_PACKET).unwrap();
    assert_eq!(
        &pe_bytes[recv_pkt_off..recv_pkt_off + EXPECTED_RECEIVED_PACKET_PROLOGUE.len()],
        &EXPECTED_RECEIVED_PACKET_PROLOGUE
    );

    // Verify every runtime-checked vtable RVA maps cleanly into the image and its slot 0 points inside .text.
    for vtable_rva in [
        RVA_UIP_CONNECTION_VTABLE,
        RVA_UNET_CONNECTION_EOS_VTABLE,
        RVA_USTEAM_NET_CONNECTION_VTABLE,
        RVA_FINTERNET_ADDR_BSD_VTABLE,
        RVA_FSOCKET_BSD_VTABLE,
    ] {
        let vt_off: usize = rva_to_file_offset(&pe_bytes, vtable_rva).unwrap();
        let slot0_va: u64 = u64::from_le_bytes([
            pe_bytes[vt_off],
            pe_bytes[vt_off + 1],
            pe_bytes[vt_off + 2],
            pe_bytes[vt_off + 3],
            pe_bytes[vt_off + 4],
            pe_bytes[vt_off + 5],
            pe_bytes[vt_off + 6],
            pe_bytes[vt_off + 7],
        ]);
        assert!(
            slot0_va > 0x1_4000_1000 && slot0_va < 0x1_4330_0000,
            "unexpected vtable slot0 VA {slot0_va:#x} for RVA {vtable_rva:#x}"
        );
    }
}
