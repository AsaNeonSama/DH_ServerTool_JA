#![cfg(windows)]

use std::path::PathBuf;

use dh_voice_client::consts::proxy::X3DAUDIO_EXPORTS;
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

type X3DAudioInitializeFn =
    unsafe extern "system" fn(speaker_channel_mask: u32, speed_of_sound: f32, instance: *mut u8);

fn locate_built_proxy_dll() -> Option<PathBuf> {
    let exe_path: PathBuf = std::env::current_exe().ok()?;
    for ancestor in exe_path.ancestors().skip(1) {
        let direct: PathBuf = ancestor.join("dh_voice_client.dll");
        if direct.is_file() {
            return Some(direct);
        }
    }

    let manifest_dir: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir: PathBuf = manifest_dir.join("..").join("..").join("target");
    [
        target_dir.join("debug").join("dh_voice_client.dll"),
        target_dir.join("release").join("dh_voice_client.dll"),
    ]
    .into_iter()
    .find(|candidate: &PathBuf| candidate.is_file())
}

#[test]
fn compiled_client_proxy_dll_exports_x3daudio_and_forwards_initialize() {
    let Some(dll_path): Option<PathBuf> = locate_built_proxy_dll() else {
        panic!("expected Cargo to build dh_voice_client.dll before running integration tests");
    };

    let mut wide_path: Vec<u16> = dll_path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .collect();
    wide_path.push(0u16);

    // SAFETY: `wide_path` is a NUL-terminated UTF-16 path to our compiled proxy DLL.
    let module: HMODULE =
        unsafe { LoadLibraryW(PCWSTR(wide_path.as_ptr())) }.expect("LoadLibraryW client proxy DLL");

    for spec in &X3DAUDIO_EXPORTS {
        // SAFETY: `module` is loaded and `spec.c_name` is a static NUL-terminated ASCII name.
        let by_name: Option<unsafe extern "system" fn() -> isize> =
            unsafe { GetProcAddress(module, PCSTR(spec.c_name.as_ptr().cast::<u8>())) };
        assert!(
            by_name.is_some(),
            "missing named export {} (@{})",
            spec.name,
            spec.ordinal
        );

        // SAFETY: `module` is loaded and `spec.ordinal` is a valid MAKEINTRESOURCEA ordinal.
        let by_ordinal: Option<unsafe extern "system" fn() -> isize> =
            unsafe { GetProcAddress(module, PCSTR(usize::from(spec.ordinal) as *const u8)) };
        assert!(
            by_ordinal.is_some(),
            "missing ordinal export @{} ({})",
            spec.ordinal,
            spec.name
        );

        assert_eq!(
            by_name.map(|func: unsafe extern "system" fn() -> isize| func as usize),
            by_ordinal.map(|func: unsafe extern "system" fn() -> isize| func as usize),
            "ordinal @{} and name {} must resolve to identical thunk",
            spec.ordinal,
            spec.name
        );
    }

    // Invoke real forwarded `X3DAudioInitialize` (@2) on both cold and hot paths to verify
    // integer + floating-point (`xmm1`) argument forwarding into `System32\X3DAudio1_7.dll`.
    // SAFETY: `module` is loaded and `X3DAudioInitialize` is export `@2`.
    let init_raw: unsafe extern "system" fn() -> isize =
        unsafe { GetProcAddress(module, PCSTR(c"X3DAudioInitialize".as_ptr().cast::<u8>())) }
            .expect("X3DAudioInitialize export");
    // SAFETY: Export `X3DAudioInitialize` matches `X3DAudioInitializeFn`.
    let x3d_init: X3DAudioInitializeFn = unsafe { std::mem::transmute(init_raw) };

    let mut handle_cold: [u8; 20] = [0u8; 20];
    let mut handle_hot: [u8; 20] = [0u8; 20];
    // SAFETY: `handle_cold` is a writable 20-byte `X3DAUDIO_HANDLE` buffer (stereo, 343.5 m/s).
    unsafe {
        x3d_init(0x3, 343.5_f32, handle_cold.as_mut_ptr());
    }
    // SAFETY: `handle_hot` exercises the already-resolved fast tail-jump path.
    unsafe {
        x3d_init(0x3, 343.5_f32, handle_hot.as_mut_ptr());
    }
    assert_ne!(
        handle_cold, [0u8; 20],
        "X3DAudioInitialize cold forward should populate X3DAUDIO_HANDLE"
    );
    assert_eq!(
        handle_cold, handle_hot,
        "X3DAudioInitialize cold and hot paths must produce identical handle bytes"
    );

    // SAFETY: `module` was loaded by `LoadLibraryW` in this test.
    unsafe {
        FreeLibrary(module).expect("FreeLibrary client proxy DLL");
    }
}
