use std::mem::size_of;
use std::path::Path;

use dh_win32::process::{
    build_command_line, find_processes_by_name, is_process_alive, list_processes,
    read_remote_peb_and_oep, spawn_suspended_console,
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
