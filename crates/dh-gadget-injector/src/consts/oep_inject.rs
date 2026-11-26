use std::mem::size_of;

use crate::structs::oep_inject::RemoteStubData;

/// Size of the x64 absolute indirect jump (`FF 25 00 00 00 00 <u64 target>`) patched at OEP.
pub const OEP_PATCH_LEN: usize = 14;
pub(crate) const OEP_PATCH_LEN_U32: u32 = 14;

/// Remote allocation size for each of the separated Data (`RW`) and Code (`RX`) pages.
pub const REMOTE_PAGE_SIZE: usize = 4096;

/// Maximum UTF-16 code units (including NUL terminator) allowed for `gadget_dll_path`
/// inside the remote stub data page.
pub const MAX_GADGET_PATH_UTF16_LEN: usize = 1024;

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
