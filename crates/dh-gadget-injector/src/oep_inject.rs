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

pub use crate::consts::oep_inject::{MAX_GADGET_PATH_UTF16_LEN, OEP_PATCH_LEN};
use crate::consts::oep_inject::{
    OEP_PATCH_LEN_U32, OFFSET_DLL_PATH_UTF16_I32, OFFSET_LOADED_MODULE_HANDLE,
    OFFSET_LOADED_MODULE_HANDLE_I32, OFFSET_LOAD_LIBRARY_W_I32, OFFSET_OEP_I32,
    OFFSET_OLD_PROTECT_I32, OFFSET_ORIGINAL_OEP_BYTES_I32, OFFSET_TMP_PROTECT_I32,
    OFFSET_VIRTUAL_PROTECT_I32, REMOTE_PAGE_SIZE,
};
use crate::error::{InjectorError, Result};
pub use crate::structs::oep_inject::{InjectedStubInfo, OepStubInjector, RemoteStubData};

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

fn asm_res<T>(res: std::result::Result<T, iced_x86::IcedError>) -> Result<T> {
    res.map_err(|err: iced_x86::IcedError| {
        InjectorError::Inject(format!("failed to assemble x64 OEP stub: {err}"))
    })
}

/// Encodes a 14-byte x64 absolute indirect jump: `FF 25 00 00 00 00 <u64 target>`.
#[must_use]
pub fn encode_abs_jmp_14(target: u64) -> [u8; OEP_PATCH_LEN] {
    let mut bytes: [u8; OEP_PATCH_LEN] = [0u8; OEP_PATCH_LEN];
    bytes[0] = 0xFF;
    bytes[1] = 0x25;
    // disp32 = 0 (bytes[2..6] remain 0x00)
    bytes[6..14].copy_from_slice(&target.to_le_bytes());
    bytes
}

/// Builds the remote `RemoteStubData` payload, validating that `gadget_dll_path` fits in
/// `MAX_GADGET_PATH_UTF16_LEN` UTF-16 code units (including NUL terminator).
pub fn build_stub_data(
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
pub fn assemble_oep_stub(data_page_addr: u64, oep_addr: u64) -> Result<Vec<u8>> {
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

pub fn resolve_kernel32_apis() -> Result<(u64, u64)> {
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
