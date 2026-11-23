use std::mem::size_of;
use std::path::{Path, PathBuf};

use dh_gadget_injector::consts::oep_inject::{
    MAX_GADGET_PATH_UTF16_LEN, OEP_PATCH_LEN, OFFSET_DLL_PATH_UTF16, OFFSET_LOADED_MODULE_HANDLE,
    OFFSET_LOAD_LIBRARY_W, OFFSET_OEP, OFFSET_OLD_PROTECT, OFFSET_ORIGINAL_OEP_BYTES,
    OFFSET_TMP_PROTECT, OFFSET_VIRTUAL_PROTECT, REMOTE_PAGE_SIZE,
};
use dh_gadget_injector::oep_inject::{
    assemble_oep_stub, build_stub_data, encode_abs_jmp_14, resolve_kernel32_apis, RemoteStubData,
};
use dh_gadget_injector::InjectorError;

const OFFSET_LOAD_LIBRARY_W_U8: u8 = 0x08;
const OFFSET_VIRTUAL_PROTECT_U8: u8 = 0x10;

#[test]
fn data_block_layout_offsets_are_aligned_and_fit_in_one_page() {
    assert_eq!(OFFSET_OEP, 0x00);
    assert_eq!(OFFSET_LOAD_LIBRARY_W, 0x08);
    assert_eq!(OFFSET_VIRTUAL_PROTECT, 0x10);
    assert_eq!(OFFSET_LOADED_MODULE_HANDLE, 0x18);
    assert_eq!(OFFSET_OLD_PROTECT, 0x20);
    assert_eq!(OFFSET_TMP_PROTECT, 0x24);
    assert_eq!(OFFSET_ORIGINAL_OEP_BYTES, 0x28);
    assert_eq!(OFFSET_DLL_PATH_UTF16, 0x40);
    assert_eq!(size_of::<RemoteStubData>(), 0x840);
    assert!(size_of::<RemoteStubData>() <= REMOTE_PAGE_SIZE);
}

#[test]
fn abs_jmp_14_encoding_matches_ff_25_zero_disp_plus_target_le() {
    let target: u64 = 0x0000_7FF7_1234_5678;
    let encoded = encode_abs_jmp_14(target);
    assert_eq!(encoded.len(), OEP_PATCH_LEN);
    assert_eq!(&encoded[0..6], &[0xFF, 0x25, 0x00, 0x00, 0x00, 0x00]);
    assert_eq!(
        u64::from_le_bytes(encoded[6..14].try_into().unwrap()),
        target
    );
}

#[test]
fn stub_data_encodes_japanese_utf16_path_and_rejects_overlong_paths() {
    let orig = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
    let jp_path = Path::new(r"C:\Users\山田太郎\AppData\Local\DH_ServerTool_JA\run_1_1\gadget.dll");
    let data = build_stub_data(0x0001_4000_1000, 0x7FF8_0001, 0x7FF8_0002, orig, jp_path)
        .expect("Japanese UTF-16 path fits in stub data");

    assert_eq!(data.oep, 0x0001_4000_1000);
    assert_eq!(data.load_library_w, 0x7FF8_0001);
    assert_eq!(data.virtual_protect, 0x7FF8_0002);
    assert_eq!(data.loaded_module_handle, 0);
    assert_eq!(&data.original_oep_bytes[..14], &orig);
    assert_eq!(&data.original_oep_bytes[14..16], &[0, 0]);

    let nul_pos = data
        .dll_path_utf16
        .iter()
        .position(|&c| c == 0)
        .expect("NUL terminator");
    let decoded = String::from_utf16(&data.dll_path_utf16[..nul_pos]).unwrap();
    assert_eq!(decoded, jp_path.to_string_lossy());

    // Exact max boundary (1023 chars + NUL == 1024) succeeds; 1024 chars + NUL == 1025 fails.
    let exact_ok = PathBuf::from("a".repeat(MAX_GADGET_PATH_UTF16_LEN - 1));
    assert!(build_stub_data(1, 2, 3, orig, &exact_ok).is_ok());

    let too_long = PathBuf::from("a".repeat(MAX_GADGET_PATH_UTF16_LEN));
    let err = build_stub_data(1, 2, 3, orig, &too_long).unwrap_err();
    assert!(matches!(
        err,
        InjectorError::PathTooLong {
            len: 1025,
            max: MAX_GADGET_PATH_UTF16_LEN,
            ..
        }
    ));
}

#[test]
fn assembled_x64_stub_obeys_abi_embeds_addresses_and_ends_with_oep_jmp() {
    let data_addr: u64 = 0x0000_0210_CAFE_0000;
    let oep_addr: u64 = 0x0000_7FF6_DEAD_BEEF;
    let code = assemble_oep_stub(data_addr, oep_addr).expect("assemble x64 OEP stub");

    // Compact single-page machine code
    assert!(
        code.len() >= 80 && code.len() <= 256,
        "unexpected stub size: {}",
        code.len()
    );

    // Prologue starts with: push rbp (55); mov rbp, rsp (48 89 E5); push rax (50)
    assert_eq!(&code[0..5], &[0x55, 0x48, 0x89, 0xE5, 0x50]);

    // Contains `mov rbx, imm64` (48 BB <data_addr LE>)
    let mut mov_rbx = [0u8; 10];
    mov_rbx[0] = 0x48;
    mov_rbx[1] = 0xBB;
    mov_rbx[2..10].copy_from_slice(&data_addr.to_le_bytes());
    assert!(
        code.windows(mov_rbx.len()).any(|w| w == mov_rbx),
        "stub must embed mov rbx, data_page_addr"
    );

    // Contains `and rsp, -16` (48 83 E4 F0) and `sub rsp, 0x20` (48 83 EC 20)
    assert!(
        code.windows(4).any(|w| w == [0x48, 0x83, 0xE4, 0xF0]),
        "stub must align RSP to 16 bytes"
    );
    assert!(
        code.windows(4).any(|w| w == [0x48, 0x83, 0xEC, 0x20]),
        "stub must allocate 32-byte Win64 shadow space"
    );

    // Contains `lea rsp, [rbp - 0x40]` (48 8D 65 C0)
    assert!(
        code.windows(4).any(|w| w == [0x48, 0x8D, 0x65, 0xC0]),
        "stub must restore RSP from rbp - 0x40 before popping registers"
    );

    // Contains indirect calls `call qword ptr [rbx + 0x08]` (FF 53 08) and `call qword ptr [rbx + 0x10]` (FF 53 10)
    assert!(
        code.windows(3)
            .any(|w| w == [0xFF, 0x53, OFFSET_LOAD_LIBRARY_W_U8]),
        "stub must call LoadLibraryW via [rbx + 0x08]"
    );
    let vp_calls = code
        .windows(3)
        .filter(|w| *w == [0xFF, 0x53, OFFSET_VIRTUAL_PROTECT_U8])
        .count();
    assert_eq!(
        vp_calls, 2,
        "stub must call VirtualProtect twice via [rbx + 0x10]"
    );

    // Ends with `pop rbp` (0x5D) immediately followed by 14-byte jump to OEP
    let jmp_start = code.len() - OEP_PATCH_LEN;
    assert_eq!(code[jmp_start - 1], 0x5D, "epilogue must end with pop rbp");
    assert_eq!(&code[jmp_start..], &encode_abs_jmp_14(oep_addr));
}

#[test]
fn resolve_kernel32_apis_returns_valid_non_null_addresses() {
    let (llw, vp) = resolve_kernel32_apis().expect("resolve kernel32 APIs");
    assert_ne!(llw, 0);
    assert_ne!(vp, 0);
    assert_ne!(llw, vp);
}
