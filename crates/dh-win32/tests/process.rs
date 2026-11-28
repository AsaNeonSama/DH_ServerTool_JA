use std::ffi::OsString;
use std::fs;
use std::mem::size_of;
use std::path::{Path, PathBuf};

use dh_win32::process::{
    build_command_line, build_unicode_environment_block, find_processes_by_name, is_process_alive,
    list_processes, read_remote_peb_and_oep, spawn_suspended, spawn_suspended_console,
    spawn_suspended_gui, SpawnOptions,
};
use dh_win32::structs::process::ProcessBasicInformation;

#[test]
fn pbi_struct_is_48_bytes_on_x64() {
    assert_eq!(size_of::<ProcessBasicInformation>(), 48);
    assert_eq!(
        std::mem::offset_of!(ProcessBasicInformation, peb_base_address),
        8
    );
}

#[test]
fn command_line_quoting_handles_spaces_and_ue_args() {
    let exe = Path::new(r"C:\Program Files\Dread Hunger\Server.exe");
    let args = vec![
        "DreadHunger".to_string(),
        "Departure_Persistent?listen?Game=/Game/Test".to_string(),
        "-log".to_string(),
        "-port=7895".to_string(),
        "arg with spaces".to_string(),
    ];
    let cmd = build_command_line(exe, &args);
    assert_eq!(
        cmd,
        r#""C:\Program Files\Dread Hunger\Server.exe" DreadHunger Departure_Persistent?listen?Game=/Game/Test -log -port=7895 "arg with spaces""#
    );
}

#[test]
fn current_process_is_alive_and_enumerable() {
    let my_pid = std::process::id();
    assert!(is_process_alive(my_pid));
    assert!(!is_process_alive(0));

    let procs = list_processes();
    let me = procs
        .iter()
        .find(|p| p.pid == my_pid)
        .expect("current test exe in snapshot");
    let by_name = find_processes_by_name(&me.name.to_ascii_uppercase());
    assert!(by_name.iter().any(|p| p.pid == my_pid));
}

#[test]
fn spawn_suspended_reads_valid_peb_and_oep() {
    let cmd = Path::new(r"C:\Windows\System32\cmd.exe");
    let args = vec!["/C".to_string(), "exit 0".to_string()];
    let proc = spawn_suspended_console(cmd, &args, None).expect("spawn cmd.exe suspended");
    let pe = read_remote_peb_and_oep(proc.process_handle_raw()).expect("read PEB and OEP");

    assert_ne!(pe.peb_address, 0);
    assert_ne!(pe.image_base, 0);
    assert_ne!(pe.entry_point_rva, 0);
    assert_eq!(pe.oep, pe.image_base + u64::from(pe.entry_point_rva));

    proc.resume().expect("resume cmd.exe");
    let code = proc.wait_exit(5000).expect("wait cmd.exe");
    assert_eq!(code, Some(0));
}

#[test]
fn spawn_and_peb_reject_invalid_inputs() {
    assert!(spawn_suspended_console(Path::new(""), &[], None).is_err());
    assert!(spawn_suspended_console(Path::new("cmd\0.exe"), &[], None).is_err());
    assert!(spawn_suspended_console(
        Path::new(r"C:\Windows\System32\cmd.exe"),
        &["/C\0bad".to_string()],
        None
    )
    .is_err());
    assert!(spawn_suspended_console(
        Path::new(r"C:\Windows\System32\cmd.exe"),
        &[],
        Some(Path::new("bad\0dir"))
    )
    .is_err());
    assert!(read_remote_peb_and_oep(0).is_err());
    assert!(read_remote_peb_and_oep(-1).is_err());
}

#[test]
fn command_line_quoting_escapes_backslashes_quotes_and_empty_args() {
    let exe = Path::new(r"C:\Dir\Server.exe");
    let args = vec![
        String::new(),
        r#"a"b"#.to_string(),
        r"trailing\".to_string(),
        r#"slash\"quote"#.to_string(),
    ];
    let cmd = build_command_line(exe, &args);
    assert_eq!(
        cmd,
        r#""C:\Dir\Server.exe" "" "a\"b" trailing\ "slash\\\"quote""#
    );
}

#[test]
fn spawn_options_defaults_and_gui_mode_work() {
    assert_eq!(SpawnOptions::default(), SpawnOptions::console());
    assert!(SpawnOptions::console().create_console);
    assert!(!SpawnOptions::gui().create_console);

    let cmd = Path::new(r"C:\Windows\System32\cmd.exe");
    let args = vec!["/C".to_string(), "exit 0".to_string()];

    let proc_gui = spawn_suspended_gui(cmd, &args, None).expect("spawn_suspended_gui");
    let pe = read_remote_peb_and_oep(proc_gui.process_handle_raw()).expect("read PEB and OEP");
    assert_ne!(pe.oep, 0);
    proc_gui.resume().expect("resume gui process");
    assert_eq!(proc_gui.wait_exit(5000).expect("wait gui process"), Some(0));

    let proc_opts =
        spawn_suspended(cmd, &args, None, &SpawnOptions::gui()).expect("spawn_suspended(gui)");
    proc_opts.resume().expect("resume opts process");
    assert_eq!(
        proc_opts.wait_exit(5000).expect("wait opts process"),
        Some(0)
    );
}

#[test]
fn spawned_process_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<dh_win32::SpawnedProcess>();
}

#[test]
fn unicode_env_block_merges_case_insensitively_sorts_and_validates() {
    let base: Vec<(OsString, OsString)> = vec![
        (OsString::from("zeta"), OsString::from("1")),
        (OsString::from("STEAMAPPID"), OsString::from("999")),
        (OsString::from("Alpha"), OsString::from("base")),
    ];
    let overrides: Vec<(String, String)> = vec![
        ("SteamAppId".to_string(), "1418630".to_string()),
        ("SteamGameId".to_string(), "1418630".to_string()),
        ("DH_JA".to_string(), "日本語環境変数".to_string()),
    ];

    let block = build_unicode_environment_block(base, &overrides).expect("build env block");
    // Must end with double-NUL terminator.
    assert!(block.len() >= 2);
    assert_eq!(block[block.len() - 2], 0);
    assert_eq!(block[block.len() - 1], 0);

    // Parse entries back out from the UTF-16LE block.
    let entries: Vec<String> = block[..block.len() - 1]
        .split(|&c| c == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf16(s).expect("valid UTF-16 entry"))
        .collect();
    assert_eq!(
        entries,
        vec![
            "Alpha=base",
            "DH_JA=日本語環境変数",
            "SteamAppId=1418630",
            "SteamGameId=1418630",
            "zeta=1",
        ]
    );

    // Invalid keys/values must be rejected.
    assert!(build_unicode_environment_block(
        Vec::<(OsString, OsString)>::new(),
        &[(String::new(), "1".to_string())]
    )
    .is_err());
    assert!(build_unicode_environment_block(
        Vec::<(OsString, OsString)>::new(),
        &[("BAD=KEY".to_string(), "1".to_string())]
    )
    .is_err());
    assert!(build_unicode_environment_block(
        Vec::<(OsString, OsString)>::new(),
        &[("BAD\0KEY".to_string(), "1".to_string())]
    )
    .is_err());
    assert!(build_unicode_environment_block(
        Vec::<(OsString, OsString)>::new(),
        &[("GOOD_KEY".to_string(), "bad\0val".to_string())]
    )
    .is_err());
}

#[test]
fn spawn_suspended_passes_env_overrides_without_mutating_parent() {
    let temp_dir: PathBuf =
        std::env::temp_dir().join(format!("dh_win32_env_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("create temp dir");
    let out_file: PathBuf = temp_dir.join("env_out.txt");

    let mut options = SpawnOptions::gui();
    options.env_overrides = vec![
        ("SteamAppId".to_string(), "1418630".to_string()),
        ("SteamGameId".to_string(), "1418630".to_string()),
        ("DH_TEST_CUSTOM_KEY".to_string(), "OK_1418630".to_string()),
    ];

    let proc = spawn_suspended(
        Path::new(r"C:\Windows\System32\cmd.exe"),
        &[
            "/C".to_string(),
            "echo %SteamAppId%:%SteamGameId%:%DH_TEST_CUSTOM_KEY%:%SystemRoot%>env_out.txt"
                .to_string(),
        ],
        Some(&temp_dir),
        &options,
    )
    .expect("spawn cmd.exe with env overrides");
    proc.resume().expect("resume cmd.exe");
    assert_eq!(proc.wait_exit(5000).expect("wait cmd.exe"), Some(0));

    let content = fs::read_to_string(&out_file).expect("read child env output");
    let _ = fs::remove_dir_all(&temp_dir);
    assert!(
        content.starts_with("1418630:1418630:OK_1418630:"),
        "child should receive SteamAppId/SteamGameId/custom env overrides; got: {content:?}"
    );
    assert!(
        !content.trim().ends_with(':'),
        "inherited SystemRoot must still be present; got: {content:?}"
    );
    assert!(
        std::env::var("DH_TEST_CUSTOM_KEY").is_err(),
        "parent process environment must not be mutated"
    );
}
