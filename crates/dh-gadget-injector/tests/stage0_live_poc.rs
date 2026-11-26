use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dh_gadget_injector::{
    spawn_and_inject, ActiveInjectionSession, InjectLaunchRequest, InjectedStubInfo, InjectorEvent,
    PatchMod,
};
use dh_win32::{find_udp_listener_pid, is_process_alive, is_tcp_port_listening, UdpLookupResult};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

const CONSOLE_BRIDGE_PORT: u16 = 17893;
const GAME_SERVER_PORT: u16 = 17899;

fn sha256_file_hex(path: &Path) -> std::io::Result<String> {
    let mut file: File = File::open(path)?;
    let mut hasher: Sha256 = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    let digest: [u8; 32] = hasher.finalize().into();
    let mut out: String = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut out, "{b:02x}");
    }
    Ok(out)
}

fn encode_utf16le_null(text: &str) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity((text.len() + 1) * 2);
    for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Stage 0 end-to-end live `PoC` against the vanilla `DreadHunger` dedicated server binary.
///
/// Run explicitly with:
/// `cargo test -p dh-gadget-injector --test stage0_live_poc -- --ignored --nocapture`
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "launches real DreadHungerServer-Win64-Shipping.exe (run explicitly with --ignored --nocapture)"]
async fn stage0_live_oep_gadget_poc_against_dread_hunger_server() {
    let manifest_dir: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let controller_dir: &Path = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("controller workspace root");
    let workspace_root: &Path = controller_dir.parent().expect("DreadHunger workspace root");

    let server_exe: PathBuf = workspace_root
        .join("references")
        .join("WindowsServer")
        .join("DreadHunger")
        .join("Binaries")
        .join("Win64")
        .join("DreadHungerServer-Win64-Shipping.exe");
    assert!(
        server_exe.is_file(),
        "vanilla server binary not found: {}",
        server_exe.display()
    );

    // Record vanilla server binary SHA-256 before launching (must stay 100% identical).
    let sha_before: String = sha256_file_hex(&server_exe).expect("hash server exe before");
    println!("[Stage0 PoC] Server exe SHA-256 BEFORE: {sha_before}");

    // Place cache (Japanese path), cwd, UE4 userdir, and patch_runtime.log strictly inside %TEMP%.
    let temp_root: PathBuf =
        std::env::temp_dir().join(format!("dh_stage0_poc_テスト日本語_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_root);
    let cache_dir: PathBuf = temp_root.join("キャッシュ_frida");
    let cwd_dir: PathBuf = temp_root.join("cwd");
    let ue_user_dir: PathBuf = temp_root.join("ue_saved");
    fs::create_dir_all(&cache_dir).expect("create cache_dir");
    fs::create_dir_all(&cwd_dir).expect("create cwd_dir");
    fs::create_dir_all(&ue_user_dir).expect("create ue_user_dir");

    let patch_log_path: PathBuf = temp_root.join("patch_runtime.log");
    fs::write(
        &patch_log_path,
        "=== Stage 0 Live PoC patch_runtime.log ===\n",
    )
    .expect("init patch_runtime.log");

    // Load the 3 newly renamed Japanese MOD scripts (read-only from controller/assets/mods/).
    let mods_root: PathBuf = controller_dir.join("assets").join("mods");
    let mod_specs: [(&str, &str, &str); 3] = [
        (
            "default:console",
            "default/コンソール連携.js",
            "コンソール連携",
        ),
        (
            "default:network_fix",
            "default/接続保護_乱入防止.js",
            "接続保護・乱入防止",
        ),
        ("single:vomit_modify", "single/毒耐性.js", "毒耐性"),
    ];

    let mut mods: Vec<PatchMod> = Vec::with_capacity(mod_specs.len() + 1);
    for (id, rel_path, label) in mod_specs {
        let full_path: PathBuf = mods_root.join(rel_path);
        let source: String = fs::read_to_string(&full_path).unwrap_or_else(|e: std::io::Error| {
            panic!("failed to read {}: {e}", full_path.display())
        });
        mods.push(PatchMod {
            id: id.to_string(),
            file_name: rel_path.to_string(),
            label: label.to_string(),
            source,
        });
    }

    // Add a tiny read-only Stage 0 probe script verifying compat shim + console/send streaming + gadget.dll presence.
    mods.push(PatchMod {
        id: "poc:probe".to_string(),
        file_name: "stage0_probe.js".to_string(),
        label: "Stage0 Probe".to_string(),
        source: concat!(
            "var base = Module.findBaseAddress('DreadHungerServer-Win64-Shipping.exe');\n",
            "var gad = Process.findModuleByName('gadget.dll');\n",
            "console.log('STAGE0_CONSOLE_PROBE_OK base=' + base + ' gadget=' + (gad ? gad.base : 'null'));\n",
            "send({ marker: 'STAGE0_SEND_PROBE_OK', base: String(base), gadgetLoaded: !!gad });\n"
        )
        .to_string(),
    });

    let mut tokens: HashMap<String, String> = HashMap::new();
    tokens.insert(
        "__CONSOLE_PATCH_PORT__".to_string(),
        CONSOLE_BRIDGE_PORT.to_string(),
    );

    let events: Arc<Mutex<Vec<InjectorEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let events_cb: Arc<Mutex<Vec<InjectorEvent>>> = Arc::clone(&events);
    let log_path_cb: PathBuf = patch_log_path.clone();
    let log_file_lock: Arc<Mutex<()>> = Arc::new(Mutex::new(()));

    let mut req: InjectLaunchRequest = InjectLaunchRequest::new(
        server_exe.clone(),
        vec![
            "DreadHunger".to_string(),
            "Departure_Persistent?listen".to_string(),
            "-noLogFiles".to_string(),
            format!("-Port={GAME_SERVER_PORT}"),
            format!("-userdir={}", ue_user_dir.display()),
        ],
        mods,
    );
    req.cwd = Some(cwd_dir.clone());
    req.tokens = tokens;
    req.patch_log_path = patch_log_path.clone();
    req.round = 1;
    req.init_timeout = Duration::from_secs(30);
    req.cache_dir_override = Some(cache_dir.clone());
    req.on_event = Some(Arc::new(move |ev: InjectorEvent| {
        if let InjectorEvent::Log {
            level,
            ref file_name,
            ref message,
        } = ev
        {
            if let Ok(_guard) = log_file_lock.lock() {
                if let Ok(mut f) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path_cb)
                {
                    let _ = writeln!(f, "[{}] [{}] {}", level.as_str(), file_name, message);
                }
            }
        }
        if let Ok(mut list) = events_cb.lock() {
            list.push(ev);
        }
    }));

    println!(
        "[Stage0 PoC] Spawning suspended DreadHungerServer and injecting Frida Gadget at OEP..."
    );
    let session: ActiveInjectionSession = spawn_and_inject(req)
        .await
        .expect("spawn_and_inject must succeed with INIT_OK");

    let pid: u32 = session.pid();
    let stub: InjectedStubInfo = *session.stub_info();
    let gadget_hmodule: u64 = session
        .loaded_module_handle()
        .expect("read_loaded_module_handle must succeed while process is alive");
    assert_ne!(
        gadget_hmodule, 0,
        "LoadLibraryW(gadget.dll) HMODULE must be non-zero"
    );
    let round_dir: PathBuf = session
        .round_dir()
        .expect("round_dir present")
        .to_path_buf();
    println!(
        "[Stage0 PoC] INIT_OK! PID={pid}, HMODULE(gadget.dll)={gadget_hmodule:#x}, OEP={:#x}, ImageBase={:#x}, DataPage={:#x}, CodePage={:#x}",
        stub.oep, stub.pe_entry.image_base, stub.data_page_addr, stub.code_page_addr
    );
    assert!(
        session.is_alive(),
        "server process must be alive after INIT_OK"
    );
    assert!(round_dir.is_dir(), "round_dir must exist while running");

    // Verify MOD_OK for every script + INIT_OK + zero InitError
    {
        let captured: std::sync::MutexGuard<'_, Vec<InjectorEvent>> =
            events.lock().expect("lock events");
        for expected in [
            "default/コンソール連携.js",
            "default/接続保護_乱入防止.js",
            "single/毒耐性.js",
            "stage0_probe.js",
        ] {
            assert!(
                captured.iter().any(|e: &InjectorEvent| matches!(
                    e,
                    InjectorEvent::ModLoaded { file_name } if file_name == expected
                )),
                "missing MOD_OK for {expected}; captured={captured:?}"
            );
        }
        assert!(
            captured
                .iter()
                .any(|e: &InjectorEvent| matches!(e, InjectorEvent::InitOk)),
            "missing InitOk event"
        );
        assert!(
            !captured
                .iter()
                .any(|e: &InjectorEvent| matches!(e, InjectorEvent::InitError { .. })),
            "unexpected InitError event: {captured:?}"
        );
    }

    // Confirm patch_runtime.log was written with both console.log and send() outputs.
    let log_deadline: Instant = Instant::now() + Duration::from_secs(5);
    let mut patch_log_content: String = String::new();
    while Instant::now() < log_deadline {
        patch_log_content = fs::read_to_string(&patch_log_path).unwrap_or_default();
        if patch_log_content.contains("STAGE0_CONSOLE_PROBE_OK")
            && patch_log_content.contains("STAGE0_SEND_PROBE_OK")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!("[Stage0 PoC] patch_runtime.log:\n{patch_log_content}");
    assert!(
        patch_log_content.contains("STAGE0_CONSOLE_PROBE_OK"),
        "patch_runtime.log missing console.log probe output"
    );
    assert!(
        patch_log_content.contains("STAGE0_SEND_PROBE_OK"),
        "patch_runtime.log missing send() probe output"
    );
    let expected_base_hex: String = format!("{:#x}", stub.pe_entry.image_base);
    assert!(
        patch_log_content
            .to_ascii_lowercase()
            .contains(&expected_base_hex.to_ascii_lowercase()),
        "probe base address should match PEB image_base {expected_base_hex}"
    );

    // Wait for UE4 to boot into Departure_Persistent and execute ADH_GameState::Tick,
    // which starts the console bridge TCP listener on 127.0.0.1:17893 (and binds UDP 17899).
    let bridge_deadline: Instant = Instant::now() + Duration::from_secs(45);
    let mut bridge_ready: bool = false;
    let mut udp_ready: bool = false;
    while Instant::now() < bridge_deadline {
        assert!(
            session.is_alive(),
            "DreadHungerServer exited prematurely while waiting for TCP bridge {CONSOLE_BRIDGE_PORT}"
        );
        if !udp_ready {
            let udp: UdpLookupResult = find_udp_listener_pid(GAME_SERVER_PORT, Some(pid));
            if udp.pid == Some(pid) {
                udp_ready = true;
                println!("[Stage0 PoC] UDP port {GAME_SERVER_PORT} confirmed bound by PID {pid}");
            }
        }
        if is_tcp_port_listening(CONSOLE_BRIDGE_PORT) {
            bridge_ready = true;
            println!(
                "[Stage0 PoC] Console TCP bridge 127.0.0.1:{CONSOLE_BRIDGE_PORT} confirmed listening!"
            );
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(
        bridge_ready,
        "timed out waiting for コンソール連携.js TCP bridge on 127.0.0.1:{CONSOLE_BRIDGE_PORT}"
    );

    // Send a test UTF-16LE + NUL JSON command to 127.0.0.1:17893
    let test_cmd_json: &str =
        r#"{"action":"send_thrall_message","targetRole":"Captain","message":"Stage0 PoC テスト"}"#;
    let payload_bytes: Vec<u8> = encode_utf16le_null(test_cmd_json);
    let mut stream: TcpStream = TcpStream::connect(("127.0.0.1", CONSOLE_BRIDGE_PORT))
        .await
        .expect("connect to 127.0.0.1:17893 console bridge");
    stream
        .write_all(&payload_bytes)
        .await
        .expect("write UTF-16LE JSON payload");
    stream.shutdown().await.expect("shutdown TCP write half");
    println!(
        "[Stage0 PoC] Sent {} bytes UTF-16LE JSON command to 127.0.0.1:{CONSOLE_BRIDGE_PORT}",
        payload_bytes.len()
    );

    // Give ADH_GameState::Tick a moment to dequeue & process the command, confirm process remains healthy.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(
        session.is_alive(),
        "server crashed after receiving UTF-16LE console TCP command"
    );

    // Drop session WITHOUT manual terminate(): verify Win32 JobObject (KILL_ON_JOB_CLOSE) kills server cleanly.
    println!("[Stage0 PoC] Dropping ActiveInjectionSession to verify JobObject kill-on-close...");
    drop(session);

    let kill_deadline: Instant = Instant::now() + Duration::from_secs(5);
    let mut killed: bool = false;
    while Instant::now() < kill_deadline {
        if !is_process_alive(pid) {
            killed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        killed,
        "PID {pid} remained alive after ActiveInjectionSession / JobObject drop"
    );
    println!("[Stage0 PoC] Confirmed PID {pid} terminated cleanly by JobObject on drop!");

    // Confirm per-round temp dir was cleaned up on drop
    assert!(
        !round_dir.exists(),
        "per-round staging dir {} should be removed on drop",
        round_dir.display()
    );

    // Confirm vanilla server exe SHA-256 is 100% identical after live run
    let sha_after: String = sha256_file_hex(&server_exe).expect("hash server exe after");
    println!("[Stage0 PoC] Server exe SHA-256 AFTER : {sha_after}");
    assert_eq!(
        sha_before, sha_after,
        "vanilla server binary SHA-256 MUST be identical before and after"
    );

    // Clean up %TEMP% staging directory
    let _ = fs::remove_dir_all(&temp_root);
    println!("[Stage0 PoC] ALL CHECKS PASSED!");
}
