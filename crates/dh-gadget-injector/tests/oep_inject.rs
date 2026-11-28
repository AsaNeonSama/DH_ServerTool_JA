use std::mem::size_of;
use std::path::{Path, PathBuf};

use dh_gadget_injector::consts::oep_inject::{
    MAX_DLL_PATH_UTF16_LEN, MAX_GADGET_PATH_UTF16_LEN, MAX_INJECT_DLLS, MULTI_STUB_HEADER_SIZE,
    OEP_PATCH_LEN, OFFSET_DLL_PATH_UTF16, OFFSET_LOADED_MODULE_HANDLE, OFFSET_LOAD_LIBRARY_W,
    OFFSET_MULTI_COMPLETED_COUNT, OFFSET_MULTI_DLL_COUNT, OFFSET_MULTI_LOADED_HANDLES,
    OFFSET_MULTI_LOAD_LIBRARY_W, OFFSET_MULTI_OEP, OFFSET_MULTI_OLD_PROTECT,
    OFFSET_MULTI_ORIGINAL_OEP_BYTES, OFFSET_MULTI_PATH_OFFSETS, OFFSET_MULTI_STUB_FINISHED,
    OFFSET_MULTI_TMP_PROTECT, OFFSET_MULTI_VIRTUAL_PROTECT, OFFSET_OEP, OFFSET_OLD_PROTECT,
    OFFSET_ORIGINAL_OEP_BYTES, OFFSET_TMP_PROTECT, OFFSET_VIRTUAL_PROTECT, REMOTE_PAGE_SIZE,
};
use dh_gadget_injector::oep_inject::{
    assemble_multi_dll_oep_stub, assemble_oep_stub, build_multi_stub_data, build_stub_data,
    encode_abs_jmp_14, resolve_kernel32_apis, MultiDllStubHeader, RemoteStubData,
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
    let jp_path = Path::new(r"C:\TestRoot\山田太郎\AppData\Local\DH_Tool_JA\run_1_1\gadget.dll");
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

    // Empty path must be rejected before remote memory allocation.
    assert!(build_stub_data(1, 2, 3, orig, Path::new("")).is_err());
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

#[test]
fn multi_dll_header_layout_offsets_are_aligned() {
    assert_eq!(OFFSET_MULTI_OEP, 0x00);
    assert_eq!(OFFSET_MULTI_LOAD_LIBRARY_W, 0x08);
    assert_eq!(OFFSET_MULTI_VIRTUAL_PROTECT, 0x10);
    assert_eq!(OFFSET_MULTI_DLL_COUNT, 0x18);
    assert_eq!(OFFSET_MULTI_COMPLETED_COUNT, 0x1C);
    assert_eq!(OFFSET_MULTI_OLD_PROTECT, 0x20);
    assert_eq!(OFFSET_MULTI_TMP_PROTECT, 0x24);
    assert_eq!(OFFSET_MULTI_ORIGINAL_OEP_BYTES, 0x28);
    assert_eq!(OFFSET_MULTI_STUB_FINISHED, 0x38);
    assert_eq!(OFFSET_MULTI_PATH_OFFSETS, 0x40);
    assert_eq!(OFFSET_MULTI_LOADED_HANDLES, 0xC0);
    assert_eq!(size_of::<MultiDllStubHeader>(), 0x1C0);
    assert_eq!(MULTI_STUB_HEADER_SIZE, 0x1C0);
    assert!(size_of::<MultiDllStubHeader>() < REMOTE_PAGE_SIZE);
}

#[test]
fn multi_dll_stub_data_packs_japanese_paths_supports_multipage_and_enforces_limits() {
    let orig = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
    let paths = vec![
        PathBuf::from(r"C:\Games\日本語フォルダ\eos_fix.dll"),
        PathBuf::from(r"C:\Games\ボイス\dh_voice_client.dll"),
        PathBuf::from(r"C:\Games\カスタム\gadget.dll"),
    ];

    let payload = build_multi_stub_data(0x0001_4000_1000, 0x7FF8_0001, 0x7FF8_0002, orig, &paths)
        .expect("build multi-DLL payload");
    assert_eq!(payload.dll_count, 3);
    assert_eq!(payload.alloc_size, REMOTE_PAGE_SIZE);
    assert!(payload.bytes.len() <= payload.alloc_size);

    let dll_count = u32::from_le_bytes(
        payload.bytes[OFFSET_MULTI_DLL_COUNT..OFFSET_MULTI_DLL_COUNT + 4]
            .try_into()
            .unwrap(),
    );
    assert_eq!(dll_count, 3);

    for (idx, expected) in paths.iter().enumerate() {
        let off_pos = OFFSET_MULTI_PATH_OFFSETS + idx * 4;
        let path_off =
            u32::from_le_bytes(payload.bytes[off_pos..off_pos + 4].try_into().unwrap()) as usize;
        assert!(
            path_off.is_multiple_of(4),
            "path offset {idx} must be 4-byte aligned"
        );
        assert!(path_off >= MULTI_STUB_HEADER_SIZE);

        let mut units = Vec::new();
        let mut cursor = path_off;
        loop {
            let unit = u16::from_le_bytes([payload.bytes[cursor], payload.bytes[cursor + 1]]);
            cursor += 2;
            if unit == 0 {
                break;
            }
            units.push(unit);
        }
        let decoded = String::from_utf16(&units).expect("valid UTF-16LE path");
        assert_eq!(decoded, expected.to_string_lossy());
    }

    // Multi-page allocation check: 32 long paths exceed 4096 bytes and round up to pages.
    let long_paths: Vec<PathBuf> = (0..MAX_INJECT_DLLS)
        .map(|i| PathBuf::from(format!("C:\\dir\\{}_{}.dll", "あ".repeat(400), i)))
        .collect();
    let big_payload = build_multi_stub_data(1, 2, 3, orig, &long_paths)
        .expect("MAX_INJECT_DLLS long paths succeed with multi-page allocation");
    assert_eq!(big_payload.dll_count as usize, MAX_INJECT_DLLS);
    assert!(big_payload.alloc_size > REMOTE_PAGE_SIZE);
    assert!(big_payload.alloc_size.is_multiple_of(REMOTE_PAGE_SIZE));
    assert!(big_payload.bytes.len() <= big_payload.alloc_size);

    // Reject empty DLL slice
    assert!(build_multi_stub_data(1, 2, 3, orig, &[]).is_err());

    // Reject exceeding MAX_INJECT_DLLS
    let too_many: Vec<PathBuf> = (0..=MAX_INJECT_DLLS)
        .map(|i| PathBuf::from(format!("C:\\d{i}.dll")))
        .collect();
    let err = build_multi_stub_data(1, 2, 3, orig, &too_many).unwrap_err();
    assert!(matches!(
        err,
        InjectorError::TooManyDlls {
            count,
            max: MAX_INJECT_DLLS,
        } if count == MAX_INJECT_DLLS + 1
    ));

    // Reject empty path entry and overlong path entry
    assert!(build_multi_stub_data(1, 2, 3, orig, &[PathBuf::from("")]).is_err());
    let overlong = PathBuf::from("a".repeat(MAX_DLL_PATH_UTF16_LEN));
    let len_err = build_multi_stub_data(1, 2, 3, orig, &[overlong]).unwrap_err();
    assert!(matches!(
        len_err,
        InjectorError::PathTooLong {
            max: MAX_DLL_PATH_UTF16_LEN,
            ..
        }
    ));
}

#[test]
fn assembled_multi_dll_stub_obeys_abi_embeds_addresses_and_ends_with_oep_jmp() {
    let data_addr: u64 = 0x0000_0210_CAFE_0000;
    let oep_addr: u64 = 0x0000_7FF6_DEAD_BEEF;
    let code =
        assemble_multi_dll_oep_stub(data_addr, oep_addr).expect("assemble multi-DLL OEP stub");

    assert!(
        code.len() >= 100 && code.len() <= 320,
        "unexpected multi-DLL stub size: {}",
        code.len()
    );
    assert!(code.len() <= REMOTE_PAGE_SIZE);

    // Prologue starts with: push rbp (55); mov rbp, rsp (48 89 E5); push rax (50)
    assert_eq!(&code[0..5], &[0x55, 0x48, 0x89, 0xE5, 0x50]);

    // Contains `mov rbx, imm64` (48 BB <data_addr LE>)
    let mut mov_rbx = [0u8; 10];
    mov_rbx[0] = 0x48;
    mov_rbx[1] = 0xBB;
    mov_rbx[2..10].copy_from_slice(&data_addr.to_le_bytes());
    assert!(
        code.windows(mov_rbx.len()).any(|w| w == mov_rbx),
        "multi-DLL stub must embed mov rbx, data_page_addr"
    );

    // Align RSP to 16 bytes & allocate 32-byte Win64 shadow space
    assert!(code.windows(4).any(|w| w == [0x48, 0x83, 0xE4, 0xF0]));
    assert!(code.windows(4).any(|w| w == [0x48, 0x83, 0xEC, 0x20]));

    // Restores RSP via `lea rsp, [rbp - 0x50]` (48 8D 65 B0)
    assert!(
        code.windows(4).any(|w| w == [0x48, 0x8D, 0x65, 0xB0]),
        "multi-DLL stub must restore RSP from rbp - 0x50"
    );

    // Calls LoadLibraryW via [rbx + 0x08] and VirtualProtect twice via [rbx + 0x10]
    assert!(code
        .windows(3)
        .any(|w| w == [0xFF, 0x53, OFFSET_LOAD_LIBRARY_W_U8]));
    let vp_calls = code
        .windows(3)
        .filter(|w| *w == [0xFF, 0x53, OFFSET_VIRTUAL_PROTECT_U8])
        .count();
    assert_eq!(vp_calls, 2);

    // Ends with `pop rbp` (0x5D) immediately followed by 14-byte jump to OEP
    let jmp_start = code.len() - OEP_PATCH_LEN;
    assert_eq!(code[jmp_start - 1], 0x5D);
    assert_eq!(&code[jmp_start..], &encode_abs_jmp_14(oep_addr));
}
