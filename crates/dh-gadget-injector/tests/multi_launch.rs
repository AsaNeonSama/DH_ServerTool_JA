use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use dh_gadget_injector::{
    spawn_and_inject, ActiveInjectionSession, DllInjectionEvent, InjectLaunchRequest,
    InjectorError, InjectorEvent, PatchMod, MAX_INJECT_DLLS,
};
use dh_win32::{is_process_alive, terminate_process};

static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(1);

fn make_japanese_temp_dir(tag: &str) -> std::io::Result<PathBuf> {
    let seq: u64 = TEST_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "dh_multi_launch_{tag}_テスト日本語_{}_{seq}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn cmd_sleep_args() -> Vec<String> {
    vec!["/C".to_string(), "ping -n 15 127.0.0.1 >NUL".to_string()]
}

fn wait_until_dead(pid: u32) -> bool {
    let deadline: Instant = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if !is_process_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    !is_process_alive(pid)
}

#[tokio::test]
async fn plain_gui_launch_has_zero_injection_overhead_and_survives_detach() {
    let cmd: PathBuf = PathBuf::from(r"C:\Windows\System32\cmd.exe");
    let req: InjectLaunchRequest =
        InjectLaunchRequest::for_gui_client(cmd, cmd_sleep_args(), Vec::new(), Vec::new());

    let session: ActiveInjectionSession = spawn_and_inject(req).await.expect("plain GUI launch");
    let pid: u32 = session.pid();
    assert!(session.is_alive());
    assert!(!session.has_injected_stub());
    assert!(session.round_dir().is_none());
    assert_eq!(session.dll_results(), []);
    assert_eq!(session.loaded_module_handle(), None);
    assert_eq!(session.try_exit_code().expect("try_exit_code"), None);

    // Detaching (or dropping with attach_job_object == false) MUST NOT kill the process.
    session.detach().await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    let still_alive: bool = is_process_alive(pid);
    let _ = terminate_process(pid);
    assert!(
        still_alive,
        "detached GUI process (PID {pid}) must stay alive after session detach"
    );
    assert!(wait_until_dead(pid));
}

#[tokio::test]
async fn preflight_rejects_missing_dll_and_too_many_dlls() {
    let cmd: PathBuf = PathBuf::from(r"C:\Windows\System32\cmd.exe");

    // Missing DLL must fail before spawning.
    let missing_req: InjectLaunchRequest = InjectLaunchRequest::for_gui_client(
        cmd.clone(),
        cmd_sleep_args(),
        vec![PathBuf::from(
            r"C:\nonexistent_dh_test_dir\missing_テスト.dll",
        )],
        Vec::new(),
    );
    let missing_err: InjectorError = spawn_and_inject(missing_req).await.unwrap_err();
    assert!(matches!(missing_err, InjectorError::DllNotFound { .. }));

    // MAX_INJECT_DLLS + 1 DLLs must fail with TooManyDlls.
    let too_many_dlls: Vec<PathBuf> = (0..=MAX_INJECT_DLLS)
        .map(|i| PathBuf::from(format!("dummy_{i}.dll")))
        .collect();
    let too_many_req: InjectLaunchRequest = InjectLaunchRequest::for_gui_client(
        cmd.clone(),
        cmd_sleep_args(),
        too_many_dlls,
        Vec::new(),
    );
    let too_many_err: InjectorError = spawn_and_inject(too_many_req).await.unwrap_err();
    assert!(matches!(
        too_many_err,
        InjectorError::TooManyDlls {
            count,
            max: MAX_INJECT_DLLS,
        } if count == MAX_INJECT_DLLS + 1
    ));

    // MAX_INJECT_DLLS user DLLs + 1 JS mod exceeds total MAX_INJECT_DLLS because gadget.dll uses 1 slot.
    let max_dlls: Vec<PathBuf> = (0..MAX_INJECT_DLLS)
        .map(|i| PathBuf::from(format!("dummy_{i}.dll")))
        .collect();
    let js_mod: PatchMod = PatchMod {
        id: "t".to_string(),
        file_name: "t.js".to_string(),
        label: "t".to_string(),
        source: String::new(),
    };
    let overflow_combo: InjectLaunchRequest =
        InjectLaunchRequest::for_gui_client(cmd, cmd_sleep_args(), max_dlls, vec![js_mod]);
    let combo_err: InjectorError = spawn_and_inject(overflow_combo).await.unwrap_err();
    assert!(matches!(
        combo_err,
        InjectorError::TooManyDlls {
            count,
            max: MAX_INJECT_DLLS,
        } if count == MAX_INJECT_DLLS + 1
    ));
}

#[tokio::test]
async fn dll_only_multi_dll_oep_injection_loads_japanese_paths_without_frida_gadget() {
    let temp_dir: PathBuf = make_japanese_temp_dir("dll_only").expect("create temp dir");
    let src_dll_a: &Path = Path::new(r"C:\Windows\System32\version.dll");
    let src_dll_b: &Path = Path::new(r"C:\Windows\System32\msimg32.dll");
    let dll_a: PathBuf = temp_dir.join("テストDLL_壱.dll");
    let dll_b: PathBuf = temp_dir.join("テストDLL_弐.dll");
    fs::copy(src_dll_a, &dll_a).expect("copy version.dll to Japanese path");
    fs::copy(src_dll_b, &dll_b).expect("copy msimg32.dll to Japanese path");

    let dll_events: Arc<StdMutex<Vec<DllInjectionEvent>>> = Arc::new(StdMutex::new(Vec::new()));
    let dll_events_cb: Arc<StdMutex<Vec<DllInjectionEvent>>> = Arc::clone(&dll_events);

    let mut req: InjectLaunchRequest = InjectLaunchRequest::for_gui_client(
        PathBuf::from(r"C:\Windows\System32\cmd.exe"),
        cmd_sleep_args(),
        vec![dll_a.clone(), dll_b.clone()],
        Vec::new(),
    );
    req.init_timeout = Duration::from_secs(10);
    req.on_dll_event = Some(Arc::new(move |ev: DllInjectionEvent| {
        dll_events_cb.lock().unwrap().push(ev);
    }));

    let session: ActiveInjectionSession = spawn_and_inject(req)
        .await
        .expect("DLL-only multi-DLL OEP injection");
    let pid: u32 = session.pid();
    assert!(session.is_alive());
    assert!(session.has_injected_stub());
    assert!(
        session.round_dir().is_none(),
        "DLL-only launch must not extract Frida Gadget"
    );

    let results = session.dll_results();
    assert_eq!(results.len(), 2);
    assert!(results[0].is_loaded(), "DLL #0 must have non-zero HMODULE");
    assert!(results[1].is_loaded(), "DLL #1 must have non-zero HMODULE");
    assert_ne!(results[0].module_handle, results[1].module_handle);

    let captured = dll_events.lock().unwrap().clone();
    assert_eq!(captured.len(), 2);
    assert!(matches!(
        captured[0],
        DllInjectionEvent::Loaded {
            index: 0,
            total: 2,
            module_handle,
            ..
        } if Some(module_handle) == results[0].module_handle
    ));
    assert!(matches!(
        captured[1],
        DllInjectionEvent::Loaded {
            index: 1,
            total: 2,
            module_handle,
            ..
        } if Some(module_handle) == results[1].module_handle
    ));

    // Dropping a detached session must keep the process alive; terminate explicitly afterwards.
    drop(session);
    tokio::time::sleep(Duration::from_millis(150)).await;
    let alive_after_drop: bool = is_process_alive(pid);
    let _ = terminate_process(pid);
    assert!(alive_after_drop);
    assert!(wait_until_dead(pid));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn dll_load_failure_is_reported_and_optionally_enforced() {
    let temp_dir: PathBuf = make_japanese_temp_dir("dll_fail").expect("create temp dir");
    let broken_dll: PathBuf = temp_dir.join("壊れたDLL.dll");
    fs::write(&broken_dll, b"NOT A VALID PE BINARY").expect("write broken dll");

    // 1) Non-strict mode (default): emits DllInjectionEvent::Failed, keeps process running.
    let events: Arc<StdMutex<Vec<DllInjectionEvent>>> = Arc::new(StdMutex::new(Vec::new()));
    let events_cb: Arc<StdMutex<Vec<DllInjectionEvent>>> = Arc::clone(&events);

    let mut req: InjectLaunchRequest = InjectLaunchRequest::for_gui_client(
        PathBuf::from(r"C:\Windows\System32\cmd.exe"),
        cmd_sleep_args(),
        vec![broken_dll.clone()],
        Vec::new(),
    );
    req.attach_job_object = true;
    req.init_timeout = Duration::from_secs(10);
    req.on_dll_event = Some(Arc::new(move |ev: DllInjectionEvent| {
        events_cb.lock().unwrap().push(ev);
    }));

    let session: ActiveInjectionSession = spawn_and_inject(req)
        .await
        .expect("non-strict DLL load failure still launches");
    assert_eq!(session.dll_results().len(), 1);
    assert!(!session.dll_results()[0].is_loaded());
    assert!(matches!(
        events.lock().unwrap().as_slice(),
        [DllInjectionEvent::Failed {
            index: 0,
            total: 1,
            ..
        }]
    ));
    session.cleanup().await;

    // 2) Strict mode (`fail_on_dll_load_error = true`): returns Err(DllLoadFailed) & tears down child.
    let mut strict_req: InjectLaunchRequest = InjectLaunchRequest::for_gui_client(
        PathBuf::from(r"C:\Windows\System32\cmd.exe"),
        cmd_sleep_args(),
        vec![broken_dll],
        Vec::new(),
    );
    strict_req.fail_on_dll_load_error = true;
    strict_req.init_timeout = Duration::from_secs(10);
    let err: InjectorError = spawn_and_inject(strict_req).await.unwrap_err();
    assert!(matches!(err, InjectorError::DllLoadFailed { .. }));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn atomic_dll_plus_frida_js_injection_loads_dll_before_js_evaluates() {
    let temp_dir: PathBuf = make_japanese_temp_dir("combo").expect("create temp dir");
    let cache_dir: PathBuf = temp_dir.join("キャッシュ");
    fs::create_dir_all(&cache_dir).expect("create cache dir");

    let user_dll: PathBuf = temp_dir.join("コンボDLL_日本語.dll");
    fs::copy(Path::new(r"C:\Windows\System32\version.dll"), &user_dll)
        .expect("copy version.dll for combo test");

    let dll_events: Arc<StdMutex<Vec<DllInjectionEvent>>> = Arc::new(StdMutex::new(Vec::new()));
    let dll_events_cb: Arc<StdMutex<Vec<DllInjectionEvent>>> = Arc::clone(&dll_events);
    let js_events: Arc<StdMutex<Vec<InjectorEvent>>> = Arc::new(StdMutex::new(Vec::new()));
    let js_events_cb: Arc<StdMutex<Vec<InjectorEvent>>> = Arc::clone(&js_events);

    let probe_mod: PatchMod = PatchMod {
        id: "combo:probe".to_string(),
        file_name: "combo_probe.js".to_string(),
        label: "Combo Probe".to_string(),
        source: concat!(
            "var base = Module.findBaseAddress('コンボDLL_日本語.dll');\n",
            "if (!base) { throw new Error('user DLL not loaded before gadget.dll'); }\n",
            "console.log('COMBO_DLL_BASE=' + base);\n"
        )
        .to_string(),
    };

    let mut req: InjectLaunchRequest = InjectLaunchRequest::for_gui_client(
        PathBuf::from(r"C:\Windows\System32\cmd.exe"),
        cmd_sleep_args(),
        vec![user_dll.clone()],
        vec![probe_mod],
    );
    req.attach_job_object = true;
    req.cache_dir_override = Some(cache_dir);
    req.patch_log_path = temp_dir.join("combo_patch.log");
    req.init_timeout = Duration::from_secs(20);
    req.on_dll_event = Some(Arc::new(move |ev: DllInjectionEvent| {
        dll_events_cb.lock().unwrap().push(ev);
    }));
    req.on_event = Some(Arc::new(move |ev: InjectorEvent| {
        js_events_cb.lock().unwrap().push(ev);
    }));

    let session: ActiveInjectionSession = spawn_and_inject(req)
        .await
        .expect("atomic DLL + Frida Gadget JS injection");

    assert!(session.is_alive());
    assert!(session.has_injected_stub());
    assert!(session.round_dir().is_some());
    assert_eq!(session.dll_results().len(), 1);
    let dll_handle: u64 = session.dll_results()[0]
        .module_handle
        .expect("user DLL HMODULE must be non-zero");
    let gadget_handle: u64 = session
        .loaded_module_handle()
        .expect("gadget.dll HMODULE readable");
    assert_ne!(gadget_handle, 0);
    assert_ne!(dll_handle, gadget_handle);

    let captured_dll = dll_events.lock().unwrap().clone();
    assert!(matches!(
        captured_dll.as_slice(),
        [DllInjectionEvent::Loaded {
            index: 0,
            total: 1,
            module_handle,
            ..
        }] if *module_handle == dll_handle
    ));

    let captured_js = js_events.lock().unwrap().clone();
    assert!(captured_js.iter().any(|e| matches!(
        e,
        InjectorEvent::ModLoaded { file_name } if file_name == "combo_probe.js"
    )));
    assert!(captured_js
        .iter()
        .any(|e| matches!(e, InjectorEvent::InitOk)));

    let expected_hex: String = format!("{dll_handle:#x}");
    assert!(
        captured_js.iter().any(|e| matches!(
            e,
            InjectorEvent::Log { message, .. }
                if message.to_ascii_lowercase().contains(&expected_hex.to_ascii_lowercase())
        )),
        "JS probe log should match exact user DLL HMODULE {expected_hex}; events={captured_js:?}"
    );

    session.cleanup().await;
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn launch_futures_and_session_are_send_for_tokio_spawn_and_tauri_commands() {
    fn assert_send<T: Send>(_: &T) {}
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<ActiveInjectionSession>();

    let req: InjectLaunchRequest = InjectLaunchRequest::for_gui_client(
        PathBuf::from("dummy.exe"),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let fut = spawn_and_inject(req.clone());
    assert_send(&fut);
    drop(fut);

    let injector = dh_gadget_injector::OepStubInjector;
    let fut_cancel = dh_gadget_injector::spawn_and_inject_with_cancel(
        &injector,
        req,
        std::future::pending::<()>(),
    );
    assert_send(&fut_cancel);
    drop(fut_cancel);
}

#[tokio::test]
async fn gui_launch_forwards_env_overrides_to_child_process() {
    let temp_dir: PathBuf = make_japanese_temp_dir("env_fwd").expect("create temp dir");
    let out_file: PathBuf = temp_dir.join("env_out.txt");

    let mut req: InjectLaunchRequest = InjectLaunchRequest::for_gui_client(
        PathBuf::from(r"C:\Windows\System32\cmd.exe"),
        vec![
            "/C".to_string(),
            "echo %SteamAppId%:%SteamGameId%>env_out.txt".to_string(),
        ],
        Vec::new(),
        Vec::new(),
    );
    req.cwd = Some(temp_dir.clone());
    req.env_overrides = vec![
        ("SteamAppId".to_string(), "1418630".to_string()),
        ("SteamGameId".to_string(), "1418630".to_string()),
    ];

    let session: ActiveInjectionSession = spawn_and_inject(req)
        .await
        .expect("plain GUI launch with env_overrides");
    let pid: u32 = session.pid();
    assert!(wait_until_dead(pid));
    session.detach().await;

    let content: String = fs::read_to_string(&out_file).expect("read child env output");
    let _ = fs::remove_dir_all(&temp_dir);
    assert_eq!(content.trim(), "1418630:1418630");
}
