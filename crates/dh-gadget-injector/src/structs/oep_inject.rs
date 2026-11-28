use dh_win32::RemotePeEntry;

use crate::consts::oep_inject::{MAX_GADGET_PATH_UTF16_LEN, MAX_INJECT_DLLS, OEP_PATCH_LEN};

/// Legacy single-DLL remote data page layout (`PAGE_READWRITE`) kept for backwards compatibility.
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

/// Multi-DLL remote data header (`PAGE_READWRITE`) read and updated by the OEP bootstrap stub.
/// Packed NUL-terminated UTF-16LE DLL paths follow immediately after this header in remote memory
/// at the byte offsets recorded in `path_offsets[0..dll_count]`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MultiDllStubHeader {
    pub oep: u64,
    pub load_library_w: u64,
    pub virtual_protect: u64,
    pub dll_count: u32,
    pub completed_count: u32,
    pub old_protect: u32,
    pub tmp_protect: u32,
    pub original_oep_bytes: [u8; 16],
    pub stub_finished: u32,
    pub(crate) _reserved: u32,
    pub path_offsets: [u32; MAX_INJECT_DLLS],
    pub loaded_handles: [u64; MAX_INJECT_DLLS],
}

/// Serialized remote `RW` data payload for multi-DLL OEP injection:
/// `[MultiDllStubHeader][packed UTF-16LE DLL paths...]`, with `alloc_size` rounded up to pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiDllStubPayload {
    pub bytes: Vec<u8>,
    pub alloc_size: usize,
    pub dll_count: u32,
}

/// Snapshot of live multi-DLL OEP bootstrap progress inspected from remote memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubProgress {
    pub completed_count: u32,
    pub stub_finished: bool,
    pub loaded_handles: Vec<u64>,
}

/// Metadata describing the remote OEP bootstrap pages installed into a suspended process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InjectedStubInfo {
    pub pe_entry: RemotePeEntry,
    pub oep: u64,
    pub data_page_addr: u64,
    pub data_page_size: usize,
    pub code_page_addr: u64,
    pub loaded_module_handle_addr: u64,
    pub loaded_handles_addr: u64,
    pub completed_count_addr: u64,
    pub stub_finished_addr: u64,
    pub dll_count: u32,
    pub original_oep_bytes: [u8; OEP_PATCH_LEN],
}

/// Default pure-Rust OEP bootstrap stub injector:
/// - Reads remote PEB + PE32+ headers to locate `OEP` and backs up its first 14 bytes.
/// - Allocates separate `PAGE_READWRITE` data and `PAGE_EXECUTE_READ` code pages (no persistent `RWX`).
/// - Patches `OEP` with a 14-byte indirect jump to the stub, which calls `LoadLibraryW` in order for
///   each configured DLL (and optional `gadget.dll`) after Loader Lock release, records every returned
///   `HMODULE`, restores the original 14 OEP bytes and page protection, and jumps seamlessly back to `OEP`.
#[derive(Debug, Default, Clone, Copy)]
pub struct OepStubInjector;
