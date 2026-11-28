use std::mem::size_of;

use crate::structs::oep_inject::{MultiDllStubHeader, RemoteStubData};

/// Size of the x64 absolute indirect jump (`FF 25 00 00 00 00 <u64 target>`) patched at OEP.
pub const OEP_PATCH_LEN: usize = 14;
pub(crate) const OEP_PATCH_LEN_U32: u32 = 14;

/// Remote allocation granularity for separated Data (`RW`) and Code (`RX`) pages.
pub const REMOTE_PAGE_SIZE: usize = 4096;

/// Maximum UTF-16 code units (including NUL terminator) allowed for a single DLL path
/// inside the remote stub data region.
pub const MAX_GADGET_PATH_UTF16_LEN: usize = 1024;

/// Alias of [`MAX_GADGET_PATH_UTF16_LEN`] for multi-DLL OEP injection paths.
pub const MAX_DLL_PATH_UTF16_LEN: usize = MAX_GADGET_PATH_UTF16_LEN;

/// Maximum number of DLLs that can be loaded in one atomic pre-OEP bootstrap
/// (when JS MODs are active, `gadget.dll` occupies 1 slot -> up to 31 user DLLs).
pub const MAX_INJECT_DLLS: usize = 32;

pub const OFFSET_OEP: usize = std::mem::offset_of!(RemoteStubData, oep);
pub const OFFSET_LOAD_LIBRARY_W: usize = std::mem::offset_of!(RemoteStubData, load_library_w);
pub const OFFSET_VIRTUAL_PROTECT: usize = std::mem::offset_of!(RemoteStubData, virtual_protect);
pub const OFFSET_LOADED_MODULE_HANDLE: usize =
    std::mem::offset_of!(RemoteStubData, loaded_module_handle);
pub const OFFSET_OLD_PROTECT: usize = std::mem::offset_of!(RemoteStubData, old_protect);
pub const OFFSET_TMP_PROTECT: usize = std::mem::offset_of!(RemoteStubData, tmp_protect);
pub const OFFSET_ORIGINAL_OEP_BYTES: usize =
    std::mem::offset_of!(RemoteStubData, original_oep_bytes);
pub const OFFSET_DLL_PATH_UTF16: usize = std::mem::offset_of!(RemoteStubData, dll_path_utf16);

pub(crate) const OFFSET_OEP_I32: i32 = 0x00;
pub(crate) const OFFSET_LOAD_LIBRARY_W_I32: i32 = 0x08;
pub(crate) const OFFSET_VIRTUAL_PROTECT_I32: i32 = 0x10;
pub(crate) const OFFSET_LOADED_MODULE_HANDLE_I32: i32 = 0x18;
pub(crate) const OFFSET_OLD_PROTECT_I32: i32 = 0x20;
pub(crate) const OFFSET_TMP_PROTECT_I32: i32 = 0x24;
pub(crate) const OFFSET_ORIGINAL_OEP_BYTES_I32: i32 = 0x28;
pub(crate) const OFFSET_DLL_PATH_UTF16_I32: i32 = 0x40;

pub const OFFSET_MULTI_OEP: usize = std::mem::offset_of!(MultiDllStubHeader, oep);
pub const OFFSET_MULTI_LOAD_LIBRARY_W: usize =
    std::mem::offset_of!(MultiDllStubHeader, load_library_w);
pub const OFFSET_MULTI_VIRTUAL_PROTECT: usize =
    std::mem::offset_of!(MultiDllStubHeader, virtual_protect);
pub const OFFSET_MULTI_DLL_COUNT: usize = std::mem::offset_of!(MultiDllStubHeader, dll_count);
pub const OFFSET_MULTI_COMPLETED_COUNT: usize =
    std::mem::offset_of!(MultiDllStubHeader, completed_count);
pub const OFFSET_MULTI_OLD_PROTECT: usize = std::mem::offset_of!(MultiDllStubHeader, old_protect);
pub const OFFSET_MULTI_TMP_PROTECT: usize = std::mem::offset_of!(MultiDllStubHeader, tmp_protect);
pub const OFFSET_MULTI_ORIGINAL_OEP_BYTES: usize =
    std::mem::offset_of!(MultiDllStubHeader, original_oep_bytes);
pub const OFFSET_MULTI_STUB_FINISHED: usize =
    std::mem::offset_of!(MultiDllStubHeader, stub_finished);
pub const OFFSET_MULTI_PATH_OFFSETS: usize = std::mem::offset_of!(MultiDllStubHeader, path_offsets);
pub const OFFSET_MULTI_LOADED_HANDLES: usize =
    std::mem::offset_of!(MultiDllStubHeader, loaded_handles);
pub const MULTI_STUB_HEADER_SIZE: usize = size_of::<MultiDllStubHeader>();

pub(crate) const OFFSET_MULTI_OEP_I32: i32 = 0x00;
pub(crate) const OFFSET_MULTI_LOAD_LIBRARY_W_I32: i32 = 0x08;
pub(crate) const OFFSET_MULTI_VIRTUAL_PROTECT_I32: i32 = 0x10;
pub(crate) const OFFSET_MULTI_DLL_COUNT_I32: i32 = 0x18;
pub(crate) const OFFSET_MULTI_COMPLETED_COUNT_I32: i32 = 0x1C;
pub(crate) const OFFSET_MULTI_OLD_PROTECT_I32: i32 = 0x20;
pub(crate) const OFFSET_MULTI_TMP_PROTECT_I32: i32 = 0x24;
pub(crate) const OFFSET_MULTI_ORIGINAL_OEP_BYTES_I32: i32 = 0x28;
pub(crate) const OFFSET_MULTI_STUB_FINISHED_I32: i32 = 0x38;
pub(crate) const OFFSET_MULTI_PATH_OFFSETS_I32: i32 = 0x40;
pub(crate) const OFFSET_MULTI_LOADED_HANDLES_I32: i32 = 0xC0;
pub(crate) const MULTI_STUB_SAVED_REGS_FRAME_OFFSET: i32 = 0x50;

const _: () = assert!(OEP_PATCH_LEN == 14);
const _: () = assert!(OFFSET_OEP == 0x00);
const _: () = assert!(OFFSET_LOAD_LIBRARY_W == 0x08);
const _: () = assert!(OFFSET_VIRTUAL_PROTECT == 0x10);
const _: () = assert!(OFFSET_LOADED_MODULE_HANDLE == 0x18);
const _: () = assert!(OFFSET_OLD_PROTECT == 0x20);
const _: () = assert!(OFFSET_TMP_PROTECT == 0x24);
const _: () = assert!(OFFSET_ORIGINAL_OEP_BYTES == 0x28);
const _: () = assert!(OFFSET_DLL_PATH_UTF16 == 0x40);
const _: () = assert!(OFFSET_OEP_I32.cast_unsigned() as usize == OFFSET_OEP);
const _: () = assert!(OFFSET_LOAD_LIBRARY_W_I32.cast_unsigned() as usize == OFFSET_LOAD_LIBRARY_W);
const _: () =
    assert!(OFFSET_VIRTUAL_PROTECT_I32.cast_unsigned() as usize == OFFSET_VIRTUAL_PROTECT);
const _: () = assert!(
    OFFSET_LOADED_MODULE_HANDLE_I32.cast_unsigned() as usize == OFFSET_LOADED_MODULE_HANDLE
);
const _: () = assert!(OFFSET_OLD_PROTECT_I32.cast_unsigned() as usize == OFFSET_OLD_PROTECT);
const _: () = assert!(OFFSET_TMP_PROTECT_I32.cast_unsigned() as usize == OFFSET_TMP_PROTECT);
const _: () =
    assert!(OFFSET_ORIGINAL_OEP_BYTES_I32.cast_unsigned() as usize == OFFSET_ORIGINAL_OEP_BYTES);
const _: () = assert!(OFFSET_DLL_PATH_UTF16_I32.cast_unsigned() as usize == OFFSET_DLL_PATH_UTF16);
const _: () = assert!(size_of::<RemoteStubData>() == 0x40 + MAX_GADGET_PATH_UTF16_LEN * 2);
const _: () = assert!(size_of::<RemoteStubData>() <= REMOTE_PAGE_SIZE);

const _: () = assert!(OFFSET_MULTI_OEP == 0x00);
const _: () = assert!(OFFSET_MULTI_LOAD_LIBRARY_W == 0x08);
const _: () = assert!(OFFSET_MULTI_VIRTUAL_PROTECT == 0x10);
const _: () = assert!(OFFSET_MULTI_DLL_COUNT == 0x18);
const _: () = assert!(OFFSET_MULTI_COMPLETED_COUNT == 0x1C);
const _: () = assert!(OFFSET_MULTI_OLD_PROTECT == 0x20);
const _: () = assert!(OFFSET_MULTI_TMP_PROTECT == 0x24);
const _: () = assert!(OFFSET_MULTI_ORIGINAL_OEP_BYTES == 0x28);
const _: () = assert!(OFFSET_MULTI_STUB_FINISHED == 0x38);
const _: () = assert!(OFFSET_MULTI_PATH_OFFSETS == 0x40);
const _: () = assert!(OFFSET_MULTI_LOADED_HANDLES == 0xC0);
const _: () = assert!(MULTI_STUB_HEADER_SIZE == 0x1C0);
const _: () = assert!(OFFSET_MULTI_OEP_I32.cast_unsigned() as usize == OFFSET_MULTI_OEP);
const _: () = assert!(
    OFFSET_MULTI_LOAD_LIBRARY_W_I32.cast_unsigned() as usize == OFFSET_MULTI_LOAD_LIBRARY_W
);
const _: () = assert!(
    OFFSET_MULTI_VIRTUAL_PROTECT_I32.cast_unsigned() as usize == OFFSET_MULTI_VIRTUAL_PROTECT
);
const _: () =
    assert!(OFFSET_MULTI_DLL_COUNT_I32.cast_unsigned() as usize == OFFSET_MULTI_DLL_COUNT);
const _: () = assert!(
    OFFSET_MULTI_COMPLETED_COUNT_I32.cast_unsigned() as usize == OFFSET_MULTI_COMPLETED_COUNT
);
const _: () =
    assert!(OFFSET_MULTI_OLD_PROTECT_I32.cast_unsigned() as usize == OFFSET_MULTI_OLD_PROTECT);
const _: () =
    assert!(OFFSET_MULTI_TMP_PROTECT_I32.cast_unsigned() as usize == OFFSET_MULTI_TMP_PROTECT);
const _: () = assert!(
    OFFSET_MULTI_ORIGINAL_OEP_BYTES_I32.cast_unsigned() as usize == OFFSET_MULTI_ORIGINAL_OEP_BYTES
);
const _: () =
    assert!(OFFSET_MULTI_STUB_FINISHED_I32.cast_unsigned() as usize == OFFSET_MULTI_STUB_FINISHED);
const _: () =
    assert!(OFFSET_MULTI_PATH_OFFSETS_I32.cast_unsigned() as usize == OFFSET_MULTI_PATH_OFFSETS);
const _: () = assert!(
    OFFSET_MULTI_LOADED_HANDLES_I32.cast_unsigned() as usize == OFFSET_MULTI_LOADED_HANDLES
);
const _: () = assert!(MULTI_STUB_HEADER_SIZE < REMOTE_PAGE_SIZE);
const _: () = assert!(
    MULTI_STUB_HEADER_SIZE + MAX_INJECT_DLLS * (MAX_DLL_PATH_UTF16_LEN * 2 + 4)
        <= u32::MAX as usize
);
