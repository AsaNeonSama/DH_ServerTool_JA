use std::ffi::c_void;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use dh_win32::{read_remote_peb_and_oep, RemotePeEntry, SpawnedProcess};
use iced_x86::code_asm::{
    ax, dword_ptr, eax, edx, ptr, qword_ptr, r10, r11, r8, r8d, r9, rax, rbp, rbx, rcx, rdx, rsp,
    word_ptr, CodeAssembler,
};
use windows::core::{s, w};
use windows::Win32::Foundation::{HANDLE, HMODULE};
use windows::Win32::System::Diagnostics::Debug::{
    FlushInstructionCache, ReadProcessMemory, WriteProcessMemory,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, VirtualProtectEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE,
    PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
};

use crate::error::{InjectorError, Result};

/// Size of the x64 absolute indirect jump (`FF 25 00 00 00 00 <u64 target>`) patched at OEP.
pub const OEP_PATCH_LEN: usize = 14;
const OEP_PATCH_LEN_U32: u32 = 14;

/// Remote allocation size for each of the separated Data (`RW`) and Code (`RX`) pages.
const REMOTE_PAGE_SIZE: usize = 4096;

/// Maximum UTF-16 code units (including NUL terminator) allowed for `gadget_dll_path`
/// inside the remote stub data page.
pub const MAX_GADGET_PATH_UTF16_LEN: usize = 1024;

/// Remote data page layout (`PAGE_READWRITE`) read and updated by the OEP bootstrap stub.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct RemoteStubData {
    pub oep: u64,
    pub load_library_w: u64,
    pub virtual_protect: u64,
    pub loaded_module_handle: u64,
    pub old_protect: u32,
    pub tmp_protect: u32,
    pub original_oep_bytes: [u8; 16],
    pub _reserved: u64,
    pub dll_path_utf16: [u16; MAX_GADGET_PATH_UTF16_LEN],
}

const OFFSET_OEP: usize = std::mem::offset_of!(RemoteStubData, oep);
const OFFSET_LOAD_LIBRARY_W: usize = std::mem::offset_of!(RemoteStubData, load_library_w);
const OFFSET_VIRTUAL_PROTECT: usize = std::mem::offset_of!(RemoteStubData, virtual_protect);
const OFFSET_LOADED_MODULE_HANDLE: usize =
    std::mem::offset_of!(RemoteStubData, loaded_module_handle);
const OFFSET_OLD_PROTECT: usize = std::mem::offset_of!(RemoteStubData, old_protect);
const OFFSET_TMP_PROTECT: usize = std::mem::offset_of!(RemoteStubData, tmp_protect);
const OFFSET_ORIGINAL_OEP_BYTES: usize = std::mem::offset_of!(RemoteStubData, original_oep_bytes);
const OFFSET_DLL_PATH_UTF16: usize = std::mem::offset_of!(RemoteStubData, dll_path_utf16);

const OFFSET_OEP_I32: i32 = 0x00;
const OFFSET_LOAD_LIBRARY_W_I32: i32 = 0x08;
const OFFSET_VIRTUAL_PROTECT_I32: i32 = 0x10;
const OFFSET_LOADED_MODULE_HANDLE_I32: i32 = 0x18;
const OFFSET_OLD_PROTECT_I32: i32 = 0x20;
const OFFSET_TMP_PROTECT_I32: i32 = 0x24;
const OFFSET_ORIGINAL_OEP_BYTES_I32: i32 = 0x28;
const OFFSET_DLL_PATH_UTF16_I32: i32 = 0x40;

const _: () = assert!(OEP_PATCH_LEN == 14);
const _: () = assert!(OFFSET_OEP == 0x00);
const _: () = assert!(OFFSET_LOAD_LIBRARY_W == 0x08);
const _: () = assert!(OFFSET_VIRTUAL_PROTECT == 0x10);
const _: () = assert!(OFFSET_LOADED_MODULE_HANDLE == 0x18);
const _: () = assert!(OFFSET_OLD_PROTECT == 0x20);
const _: () = assert!(OFFSET_TMP_PROTECT == 0x24);
const _: () = assert!(OFFSET_ORIGINAL_OEP_BYTES == 0x28);
const _: () = assert!(OFFSET_DLL_PATH_UTF16 == 0x40);
const _: () = assert!(size_of::<RemoteStubData>() == 0x40 + MAX_GADGET_PATH_UTF16_LEN * 2);
const _: () = assert!(size_of::<RemoteStubData>() <= REMOTE_PAGE_SIZE);

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

/// Strategy trait for attaching Frida Gadget into a suspended x64 target process before `ResumeThread`.
pub trait ProcessInjector {
    /// Injects `gadget_dll_path` into the suspended `process` so it loads at process startup
    /// prior to UE4 `WinMain` execution.
    fn inject_suspended(
        &self,
        process: &SpawnedProcess,
        gadget_dll_path: &Path,
    ) -> Result<InjectedStubInfo>;

    /// Reads the `HMODULE` returned by `LoadLibraryW(gadget_dll_path)` from remote memory for
    /// diagnostics if the `INIT_OK` handshake times out.
    fn read_loaded_module_handle(
        &self,
        process: &SpawnedProcess,
        info: &InjectedStubInfo,
    ) -> Option<u64>;
}

/// Default pure-Rust OEP bootstrap stub injector:
/// - Reads remote PEB + PE32+ headers to locate `OEP` and backs up its first 14 bytes.
/// - Allocates separate `PAGE_READWRITE` data and `PAGE_EXECUTE_READ` code pages (no persistent `RWX`).
/// - Patches `OEP` with a 14-byte indirect jump to the stub, which calls `LoadLibraryW(gadget_dll_path)`
///   after Loader Lock release, records the returned `HMODULE`, restores the original 14 OEP bytes and
///   page protection, and jumps seamlessly back to `OEP`.
#[derive(Debug, Default, Clone, Copy)]
pub struct OepStubInjector;

fn asm_res<T>(res: std::result::Result<T, iced_x86::IcedError>) -> Result<T> {
    res.map_err(|err: iced_x86::IcedError| {
        InjectorError::Inject(format!("failed to assemble x64 OEP stub: {err}"))
    })
}

/// Encodes a 14-byte x64 absolute indirect jump: `FF 25 00 00 00 00 <u64 target>`.
pub(crate) fn encode_abs_jmp_14(target: u64) -> [u8; OEP_PATCH_LEN] {
    let mut bytes: [u8; OEP_PATCH_LEN] = [0u8; OEP_PATCH_LEN];
    bytes[0] = 0xFF;
    bytes[1] = 0x25;
    // disp32 = 0 (bytes[2..6] remain 0x00)
    bytes[6..14].copy_from_slice(&target.to_le_bytes());
    bytes
}

/// Builds the remote `RemoteStubData` payload, validating that `gadget_dll_path` fits in
/// `MAX_GADGET_PATH_UTF16_LEN` UTF-16 code units (including NUL terminator).
pub(crate) fn build_stub_data(
    oep: u64,
    load_library_w: u64,
    virtual_protect: u64,
    original_oep_bytes: [u8; OEP_PATCH_LEN],
    gadget_dll_path: &Path,
) -> Result<RemoteStubData> {
    let wide: Vec<u16> = gadget_dll_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0u16))
        .collect();

    if wide.len() > MAX_GADGET_PATH_UTF16_LEN {
        return Err(InjectorError::PathTooLong {
            path: gadget_dll_path.to_path_buf(),
            len: wide.len(),
            max: MAX_GADGET_PATH_UTF16_LEN,
        });
    }

    let mut orig_padded: [u8; 16] = [0u8; 16];
    orig_padded[..OEP_PATCH_LEN].copy_from_slice(&original_oep_bytes);

    let mut path_buf: [u16; MAX_GADGET_PATH_UTF16_LEN] = [0u16; MAX_GADGET_PATH_UTF16_LEN];
    path_buf[..wide.len()].copy_from_slice(&wide);

    Ok(RemoteStubData {
        oep,
        load_library_w,
        virtual_protect,
        loaded_module_handle: 0,
        old_protect: 0,
        tmp_protect: 0,
        original_oep_bytes: orig_padded,
        _reserved: 0,
        dll_path_utf16: path_buf,
    })
}

/// Assembles the position-independent x64 OEP bootstrap stub obeying the Win64 ABI:
/// - Saves `rbp`, all volatile GP registers (`rax, rcx, rdx, r8, r9, r10, r11`), and `rbx`.
/// - Guarantees 16-byte stack alignment (`and rsp, -16`) + 32-byte Win64 shadow space (`sub rsp, 0x20`).
/// - Calls `LoadLibraryW(&data->dll_path_utf16)` and stores the returned `HMODULE` in
///   `data->loaded_module_handle`.
/// - Calls `VirtualProtect(oep, 14, PAGE_EXECUTE_READWRITE, &data->old_protect)`, restores the
///   original 14 bytes at `oep`, and calls `VirtualProtect(oep, 14, data->old_protect, &data->tmp_protect)`.
/// - Restores all saved registers and the original stack pointer, then transfers control to `oep`
///   via a register-free 14-byte `jmp qword ptr [rip+0]` (`FF 25 00 00 00 00 <oep>`).
pub(crate) fn assemble_oep_stub(data_page_addr: u64, oep_addr: u64) -> Result<Vec<u8>> {
    let mut a: CodeAssembler = asm_res(CodeAssembler::new(64))?;

    // Prologue: establish frame pointer so dynamic stack alignment restores exact entry RSP.
    asm_res(a.push(rbp))?;
    asm_res(a.mov(rbp, rsp))?;
    asm_res(a.push(rax))?;
    asm_res(a.push(rcx))?;
    asm_res(a.push(rdx))?;
    asm_res(a.push(r8))?;
    asm_res(a.push(r9))?;
    asm_res(a.push(r10))?;
    asm_res(a.push(r11))?;
    asm_res(a.push(rbx))?;

    // Align RSP down to a 16-byte boundary and reserve 32-byte Win64 shadow space.
    asm_res(a.and(rsp, -16i32))?;
    asm_res(a.sub(rsp, 0x20i32))?;

    // rbx = remote RemoteStubData base (non-volatile across Win64 calls).
    asm_res(a.mov(rbx, data_page_addr))?;

    // 1) HMODULE h = LoadLibraryW(&data->dll_path_utf16[0]);
    asm_res(a.lea(rcx, ptr(rbx + OFFSET_DLL_PATH_UTF16_I32)))?;
    asm_res(a.call(qword_ptr(rbx + OFFSET_LOAD_LIBRARY_W_I32)))?;
    asm_res(a.mov(qword_ptr(rbx + OFFSET_LOADED_MODULE_HANDLE_I32), rax))?;

    // 2) VirtualProtect((LPVOID)oep, 14, PAGE_EXECUTE_READWRITE, &data->old_protect);
    asm_res(a.mov(rcx, qword_ptr(rbx + OFFSET_OEP_I32)))?;
    asm_res(a.mov(edx, OEP_PATCH_LEN_U32))?;
    asm_res(a.mov(r8d, PAGE_EXECUTE_READWRITE.0))?;
    asm_res(a.lea(r9, ptr(rbx + OFFSET_OLD_PROTECT_I32)))?;
    asm_res(a.call(qword_ptr(rbx + OFFSET_VIRTUAL_PROTECT_I32)))?;

    // 3) Restore original 14 bytes at OEP (8 + 4 + 2 bytes).
    asm_res(a.mov(rcx, qword_ptr(rbx + OFFSET_OEP_I32)))?;
    asm_res(a.mov(rax, qword_ptr(rbx + OFFSET_ORIGINAL_OEP_BYTES_I32)))?;
    asm_res(a.mov(qword_ptr(rcx), rax))?;
    asm_res(a.mov(eax, dword_ptr(rbx + (OFFSET_ORIGINAL_OEP_BYTES_I32 + 8))))?;
    asm_res(a.mov(dword_ptr(rcx + 8), eax))?;
    asm_res(a.mov(ax, word_ptr(rbx + (OFFSET_ORIGINAL_OEP_BYTES_I32 + 12))))?;
    asm_res(a.mov(word_ptr(rcx + 12), ax))?;

    // 4) VirtualProtect((LPVOID)oep, 14, data->old_protect, &data->tmp_protect);
    asm_res(a.mov(rcx, qword_ptr(rbx + OFFSET_OEP_I32)))?;
    asm_res(a.mov(edx, OEP_PATCH_LEN_U32))?;
    asm_res(a.mov(r8d, dword_ptr(rbx + OFFSET_OLD_PROTECT_I32)))?;
    asm_res(a.lea(r9, ptr(rbx + OFFSET_TMP_PROTECT_I32)))?;
    asm_res(a.call(qword_ptr(rbx + OFFSET_VIRTUAL_PROTECT_I32)))?;

    // 5) Epilogue: restore stack pointer to saved rbx slot ([rbp - 8*8]) and pop all registers.
    asm_res(a.lea(rsp, ptr(rbp - 0x40)))?;
    asm_res(a.pop(rbx))?;
    asm_res(a.pop(r11))?;
    asm_res(a.pop(r10))?;
    asm_res(a.pop(r9))?;
    asm_res(a.pop(r8))?;
    asm_res(a.pop(rdx))?;
    asm_res(a.pop(rcx))?;
    asm_res(a.pop(rax))?;
    asm_res(a.pop(rbp))?;

    let mut code: Vec<u8> = asm_res(a.assemble(0))?;

    // 6) Register-free 14-byte absolute indirect jump back to restored OEP.
    code.extend_from_slice(&encode_abs_jmp_14(oep_addr));

    if code.len() > REMOTE_PAGE_SIZE {
        return Err(InjectorError::Inject(format!(
            "assembled OEP stub ({} bytes) exceeds remote page size ({REMOTE_PAGE_SIZE})",
            code.len()
        )));
    }
    Ok(code)
}

fn resolve_kernel32_apis() -> Result<(u64, u64)> {
    // SAFETY: `kernel32.dll` is loaded in every Win32 process; `w!("kernel32.dll")` is a static NUL-terminated wide string.
    let kernel32: HMODULE =
        unsafe { GetModuleHandleW(w!("kernel32.dll")) }.map_err(|e: windows::core::Error| {
            InjectorError::Inject(format!("GetModuleHandleW(kernel32.dll) failed: {e}"))
        })?;

    // SAFETY: `kernel32` is a valid module handle and `s!("LoadLibraryW")` is a static NUL-terminated ASCII string.
    let load_library_w: unsafe extern "system" fn() -> isize =
        unsafe { GetProcAddress(kernel32, s!("LoadLibraryW")) }.ok_or_else(|| {
            InjectorError::Inject("GetProcAddress(LoadLibraryW) returned NULL".to_string())
        })?;
    // SAFETY: `kernel32` is a valid module handle and `s!("VirtualProtect")` is a static NUL-terminated ASCII string.
    let virtual_protect: unsafe extern "system" fn() -> isize =
        unsafe { GetProcAddress(kernel32, s!("VirtualProtect")) }.ok_or_else(|| {
            InjectorError::Inject("GetProcAddress(VirtualProtect) returned NULL".to_string())
        })?;

    Ok((
        load_library_w as usize as u64,
        virtual_protect as usize as u64,
    ))
}

fn read_remote_exact(handle: HANDLE, addr: u64, buf: &mut [u8]) -> Result<()> {
    let mut bytes_read: usize = 0;
    // SAFETY: `handle` is a valid process handle and `buf` provides `buf.len()` writable bytes.
    unsafe {
        ReadProcessMemory(
            handle,
            addr as *const c_void,
            buf.as_mut_ptr().cast::<c_void>(),
            buf.len(),
            Some(&raw mut bytes_read),
        )
    }
    .map_err(|e: windows::core::Error| {
        InjectorError::Inject(format!(
            "ReadProcessMemory({addr:#x}, {}) failed: {e}",
            buf.len()
        ))
    })?;

    if bytes_read != buf.len() {
        return Err(InjectorError::Inject(format!(
            "short ReadProcessMemory at {addr:#x}: expected {} bytes, got {bytes_read}",
            buf.len()
        )));
    }
    Ok(())
}

fn write_remote_exact(handle: HANDLE, addr: u64, data: &[u8]) -> Result<()> {
    let mut bytes_written: usize = 0;
    // SAFETY: `handle` is a valid process handle and `data` provides `data.len()` readable bytes.
    unsafe {
        WriteProcessMemory(
            handle,
            addr as *const c_void,
            data.as_ptr().cast::<c_void>(),
            data.len(),
            Some(&raw mut bytes_written),
        )
    }
    .map_err(|e: windows::core::Error| {
        InjectorError::Inject(format!(
            "WriteProcessMemory({addr:#x}, {}) failed: {e}",
            data.len()
        ))
    })?;

    if bytes_written != data.len() {
        return Err(InjectorError::Inject(format!(
            "short WriteProcessMemory at {addr:#x}: expected {} bytes, got {bytes_written}",
            data.len()
        )));
    }
    Ok(())
}

fn alloc_remote_rw_page(handle: HANDLE) -> Result<u64> {
    // SAFETY: `handle` is a valid remote process handle; `VirtualAllocEx` allocates a fresh committed RW page in the target address space.
    let ptr: *mut c_void = unsafe {
        VirtualAllocEx(
            handle,
            None,
            REMOTE_PAGE_SIZE,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    if ptr.is_null() {
        return Err(InjectorError::Inject(format!(
            "VirtualAllocEx(PAGE_READWRITE, {REMOTE_PAGE_SIZE}) failed: {}",
            windows::core::Error::from_win32()
        )));
    }
    Ok(ptr as usize as u64)
}

fn free_remote_page(handle: HANDLE, addr: u64) {
    if addr != 0 {
        // SAFETY: `addr` is the base address of a remote allocation previously returned by `VirtualAllocEx`.
        unsafe {
            let _ = VirtualFreeEx(handle, addr as *mut c_void, 0, MEM_RELEASE);
        }
    }
}

fn protect_remote(
    handle: HANDLE,
    addr: u64,
    size: usize,
    new_protect: PAGE_PROTECTION_FLAGS,
) -> Result<PAGE_PROTECTION_FLAGS> {
    let mut old_protect: PAGE_PROTECTION_FLAGS = PAGE_PROTECTION_FLAGS::default();
    // SAFETY: `handle` is a valid remote process handle and `&raw mut old_protect` points to writable `PAGE_PROTECTION_FLAGS` storage.
    unsafe {
        VirtualProtectEx(
            handle,
            addr as *const c_void,
            size,
            new_protect,
            &raw mut old_protect,
        )
    }
    .map_err(|e: windows::core::Error| {
        InjectorError::Inject(format!(
            "VirtualProtectEx({addr:#x}, {size}, {:#x}) failed: {e}",
            new_protect.0
        ))
    })?;
    Ok(old_protect)
}

impl ProcessInjector for OepStubInjector {
    fn inject_suspended(
        &self,
        process: &SpawnedProcess,
        gadget_dll_path: &Path,
    ) -> Result<InjectedStubInfo> {
        let raw_handle: isize = process.process_handle_raw();
        if raw_handle == 0 || raw_handle == -1 {
            return Err(InjectorError::Inject("invalid process handle".to_string()));
        }
        let handle: HANDLE = HANDLE(raw_handle as *mut c_void);

        // 1. Locate remote OEP and back up original 14 bytes.
        let pe_entry: RemotePeEntry = read_remote_peb_and_oep(raw_handle)?;
        let oep: u64 = pe_entry.oep;
        let mut original_oep_bytes: [u8; OEP_PATCH_LEN] = [0u8; OEP_PATCH_LEN];
        read_remote_exact(handle, oep, &mut original_oep_bytes)?;

        // 2. Resolve kernel32!LoadLibraryW and kernel32!VirtualProtect (identical base across 64-bit processes).
        let (load_library_w, virtual_protect): (u64, u64) = resolve_kernel32_apis()?;

        // 3. Build remote data payload (validates UTF-16 path length before allocating remote memory).
        let stub_data: RemoteStubData = build_stub_data(
            oep,
            load_library_w,
            virtual_protect,
            original_oep_bytes,
            gadget_dll_path,
        )?;

        // 4. Allocate separate RW data page and RW->RX code page (never leaves a persistent RWX page).
        let data_page_addr: u64 = alloc_remote_rw_page(handle)?;
        let code_page_addr: u64 = match alloc_remote_rw_page(handle) {
            Ok(addr) => addr,
            Err(err) => {
                free_remote_page(handle, data_page_addr);
                return Err(err);
            }
        };

        let result: Result<InjectedStubInfo> = (|| -> Result<InjectedStubInfo> {
            // Write RemoteStubData into the RW data page.
            // SAFETY: RemoteStubData is #[repr(C)], fully initialized, and has zero padding bytes.
            let data_bytes: &[u8] = unsafe {
                std::slice::from_raw_parts(
                    (&raw const stub_data).cast::<u8>(),
                    size_of::<RemoteStubData>(),
                )
            };
            write_remote_exact(handle, data_page_addr, data_bytes)?;

            // Assemble and write x64 bootstrap code into the code page, then flip RW -> RX.
            let stub_code: Vec<u8> = assemble_oep_stub(data_page_addr, oep)?;
            write_remote_exact(handle, code_page_addr, &stub_code)?;
            let _: PAGE_PROTECTION_FLAGS =
                protect_remote(handle, code_page_addr, REMOTE_PAGE_SIZE, PAGE_EXECUTE_READ)?;
            // SAFETY: `handle` is valid and `[code_page_addr, code_page_addr + stub_code.len())` is a committed remote RX code range.
            unsafe {
                let _ = FlushInstructionCache(
                    handle,
                    Some(code_page_addr as *const c_void),
                    stub_code.len(),
                );
            }

            // Patch OEP with 14-byte indirect jump to code_page_addr, then restore original protection.
            let oep_jump: [u8; OEP_PATCH_LEN] = encode_abs_jmp_14(code_page_addr);
            let old_oep_prot: PAGE_PROTECTION_FLAGS =
                protect_remote(handle, oep, OEP_PATCH_LEN, PAGE_EXECUTE_READWRITE)?;
            let write_res: Result<()> = write_remote_exact(handle, oep, &oep_jump);
            let restore_res: Result<PAGE_PROTECTION_FLAGS> =
                protect_remote(handle, oep, OEP_PATCH_LEN, old_oep_prot);
            write_res?;
            let _: PAGE_PROTECTION_FLAGS = restore_res?;

            // SAFETY: `handle` is valid and `[oep, oep + OEP_PATCH_LEN)` is the just-patched executable entry range.
            unsafe { FlushInstructionCache(handle, Some(oep as *const c_void), OEP_PATCH_LEN) }
                .map_err(|e: windows::core::Error| {
                    InjectorError::Inject(format!(
                        "FlushInstructionCache(OEP={oep:#x}) failed: {e}"
                    ))
                })?;

            Ok(InjectedStubInfo {
                pe_entry,
                oep,
                data_page_addr,
                code_page_addr,
                loaded_module_handle_addr: data_page_addr + OFFSET_LOADED_MODULE_HANDLE as u64,
                original_oep_bytes,
            })
        })();

        if result.is_err() {
            free_remote_page(handle, code_page_addr);
            free_remote_page(handle, data_page_addr);
        }

        result
    }

    fn read_loaded_module_handle(
        &self,
        process: &SpawnedProcess,
        info: &InjectedStubInfo,
    ) -> Option<u64> {
        let raw_handle: isize = process.process_handle_raw();
        if raw_handle == 0 || raw_handle == -1 || info.loaded_module_handle_addr == 0 {
            return None;
        }
        let handle: HANDLE = HANDLE(raw_handle as *mut c_void);
        let mut buf: [u8; 8] = [0u8; 8];
        read_remote_exact(handle, info.loaded_module_handle_addr, &mut buf).ok()?;
        Some(u64::from_le_bytes(buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
        let jp_path =
            Path::new(r"C:\Users\山田太郎\AppData\Local\DH_ServerTool_JA\run_1_1\gadget.dll");
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
}
