use dh_win32::RemotePeEntry;

use crate::consts::oep_inject::{MAX_GADGET_PATH_UTF16_LEN, OEP_PATCH_LEN};

/// Remote data page layout (`PAGE_READWRITE`) read and updated by the OEP bootstrap stub.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RemoteStubData {
    pub oep: u64,
    pub load_library_w: u64,
    pub virtual_protect: u64,
    pub loaded_module_handle: u64,
    pub old_protect: u32,
    pub tmp_protect: u32,
    pub original_oep_bytes: [u8; 16],
    pub(crate) _reserved: u64,
    pub dll_path_utf16: [u16; MAX_GADGET_PATH_UTF16_LEN],
}

/// Metadata describing the remote OEP bootstrap pages installed into a suspended process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InjectedStubInfo {
    pub pe_entry: RemotePeEntry,
    pub oep: u64,
    pub data_page_addr: u64,
    pub code_page_addr: u64,
    pub loaded_module_handle_addr: u64,
    pub original_oep_bytes: [u8; OEP_PATCH_LEN],
}

/// Default pure-Rust OEP bootstrap stub injector:
/// - Reads remote PEB + PE32+ headers to locate `OEP` and backs up its first 14 bytes.
/// - Allocates separate `PAGE_READWRITE` data and `PAGE_EXECUTE_READ` code pages (no persistent `RWX`).
/// - Patches `OEP` with a 14-byte indirect jump to the stub, which calls `LoadLibraryW(gadget_dll_path)`
///   after Loader Lock release, records the returned `HMODULE`, restores the original 14 OEP bytes and
///   page protection, and jumps seamlessly back to `OEP`.
#[derive(Debug, Default, Clone, Copy)]
pub struct OepStubInjector;
