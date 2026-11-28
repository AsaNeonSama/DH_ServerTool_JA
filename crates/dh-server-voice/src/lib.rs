//! Dread Hunger Dedicated Server zero-extra-port DHV2 UDP voice relay DLL (`dh_voice_server.dll`).

pub mod consts;
pub mod hooks;
pub mod logger;
pub mod patcher;
pub mod relay;
pub mod structs;

pub use hooks::{install_server_hooks_if_target, last_install_report};
pub use patcher::{
    align_down, align_up, encode_detour_site_patch, encode_jmp_abs_indirect, encode_jmp_rel32,
    encode_original_trampoline, is_target_process_path, parse_pe_size_of_image,
};
pub use relay::is_valid_server_uid;
pub use structs::patch::{DetourError, HookInstallReport};
pub use structs::relay::{
    AlignedSockAddr, OutboundControlPacket, PeerEndpoint, RelayDispatch, RelayEngine,
    SocketRelayState, TokenBucket, VoicePeerEntry,
};

#[cfg(windows)]
use std::ffi::c_void;
#[cfg(windows)]
use windows::core::BOOL;
#[cfg(windows)]
use windows::Win32::Foundation::{HINSTANCE, HMODULE};
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
#[cfg(windows)]
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;

/// Windows DLL entry point.
///
/// Installs `FSocketBSD::RecvFrom` and `FSocketBSD::Close` detours immediately when loaded into
/// `DreadHungerServer-Win64-Shipping.exe` (inert if loaded into any other process).
#[cfg(windows)]
#[no_mangle]
pub extern "system" fn DllMain(module: HINSTANCE, reason: u32, _reserved: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        // SAFETY: Disables unneeded `DLL_THREAD_ATTACH`/`DLL_THREAD_DETACH` notifications for
        // our own module handle provided by the Windows loader.
        let _disabled: Result<(), windows::core::Error> =
            unsafe { DisableThreadLibraryCalls(HMODULE(module.0)) };
        let _report: HookInstallReport = install_server_hooks_if_target();
    }
    BOOL(1)
}
