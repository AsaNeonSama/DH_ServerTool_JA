#![cfg(windows)]

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use dh_client_eos_fix::consts::proxy::DSOUND_EXPORTS;
use windows::core::{BOOL, PCSTR, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

type DirectSoundEnumerateWFn = unsafe extern "system" fn(
    cb: unsafe extern "system" fn(*const c_void, PCWSTR, PCWSTR, *mut c_void) -> BOOL,
    context: *mut c_void,
) -> i32;

static ENUM_CALLBACK_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe extern "system" fn count_audio_devices_cb(
    _guid: *const c_void,
    _description: PCWSTR,
    _module: PCWSTR,
    context: *mut c_void,
) -> BOOL {
    if !context.is_null() {
        // SAFETY: `context` points to `ENUM_CALLBACK_COUNT` passed by our test below.
        let counter: &AtomicUsize = unsafe { &*(context.cast::<AtomicUsize>()) };
        counter.fetch_add(1, Ordering::Relaxed);
    }
    BOOL::from(true)
}

fn locate_built_proxy_dll() -> Option<PathBuf> {
    let exe_path: PathBuf = std::env::current_exe().ok()?;
    for ancestor in exe_path.ancestors().skip(1) {
        let direct: PathBuf = ancestor.join("dh_client_eos_fix.dll");
        if direct.is_file() {
            return Some(direct);
        }
        if ancestor
            .file_name()
            .and_then(|n: &std::ffi::OsStr| n.to_str())
            == Some("dh-client-eos-fix")
        {
            if let Ok(entries) = std::fs::read_dir(ancestor) {
                for entry in entries.flatten() {
                    let candidate: PathBuf = entry.path().join("out").join("dh_client_eos_fix.dll");
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    let manifest_dir: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir: PathBuf = manifest_dir.join("..").join("..").join("target");
    [
        target_dir.join("debug").join("dh_client_eos_fix.dll"),
        target_dir.join("release").join("dh_client_eos_fix.dll"),
    ]
    .into_iter()
    .find(|candidate: &PathBuf| candidate.is_file())
}

#[test]
fn compiled_proxy_dll_loads_exports_all_12_ordinals_and_forwards_real_dsound() {
    let Some(dll_path): Option<PathBuf> = locate_built_proxy_dll() else {
        panic!("expected Cargo to build dh_client_eos_fix.dll before running integration tests");
    };

    let mut wide_path: Vec<u16> = dll_path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .collect();
    wide_path.push(0u16);

    // SAFETY: `wide_path` is a NUL-terminated UTF-16 path to our compiled proxy DLL.
    let module: HMODULE =
        unsafe { LoadLibraryW(PCWSTR(wide_path.as_ptr())) }.expect("LoadLibraryW proxy DLL");

    for spec in &DSOUND_EXPORTS {
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

    // Invoke real forwarded `DirectSoundEnumerateW` (@3) and `DirectSoundCaptureEnumerateW` (@8)
    // through the proxy DLL into `System32\dsound.dll` to verify cold + hot paths and ABI preservation.
    ENUM_CALLBACK_COUNT.store(0, Ordering::Relaxed);
    // SAFETY: `module` is loaded and ordinal `@3` is `DirectSoundEnumerateW`.
    let enum_w_raw: unsafe extern "system" fn() -> isize =
        unsafe { GetProcAddress(module, PCSTR(3usize as *const u8)) }.expect("ordinal @3");
    // SAFETY: Export `@3` is `DirectSoundEnumerateW` matching `DirectSoundEnumerateWFn`.
    let enum_w: DirectSoundEnumerateWFn = unsafe { std::mem::transmute(enum_w_raw) };
    // SAFETY: Passing a valid callback and static counter pointer for DirectSound enumeration.
    let hr_render_cold: i32 = unsafe {
        enum_w(
            count_audio_devices_cb,
            (&raw const ENUM_CALLBACK_COUNT).cast_mut().cast::<c_void>(),
        )
    };
    // SAFETY: Passing a valid callback and static counter pointer on the resolved fast path.
    let hr_render_hot: i32 = unsafe {
        enum_w(
            count_audio_devices_cb,
            (&raw const ENUM_CALLBACK_COUNT).cast_mut().cast::<c_void>(),
        )
    };
    assert_eq!(hr_render_cold, 0, "DirectSoundEnumerateW cold path failed");
    assert_eq!(hr_render_hot, 0, "DirectSoundEnumerateW hot path failed");

    // SAFETY: `module` is loaded and ordinal `@8` is `DirectSoundCaptureEnumerateW`.
    let cap_enum_w_raw: unsafe extern "system" fn() -> isize =
        unsafe { GetProcAddress(module, PCSTR(8usize as *const u8)) }.expect("ordinal @8");
    // SAFETY: Export `@8` is `DirectSoundCaptureEnumerateW` matching `DirectSoundEnumerateWFn`.
    let cap_enum_w: DirectSoundEnumerateWFn = unsafe { std::mem::transmute(cap_enum_w_raw) };
    // SAFETY: Passing a valid callback and static counter pointer for DirectSoundCapture enumeration.
    let hr_capture: i32 = unsafe {
        cap_enum_w(
            count_audio_devices_cb,
            (&raw const ENUM_CALLBACK_COUNT).cast_mut().cast::<c_void>(),
        )
    };
    assert_eq!(hr_capture, 0, "DirectSoundCaptureEnumerateW forward failed");

    // SAFETY: `module` was loaded by `LoadLibraryW` in this test.
    unsafe {
        FreeLibrary(module).expect("FreeLibrary proxy DLL");
    }
}
