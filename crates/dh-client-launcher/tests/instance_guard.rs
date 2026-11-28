use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use dh_client_launcher::consts::messages::ERR_LAUNCH_IN_PROGRESS;
use dh_client_launcher::instance_guard::{
    find_matching_process_in, focus_existing_window, is_pid_running, target_game_exe_names,
    try_acquire_app_instance, try_acquire_named_mutex,
};
use dh_client_launcher::launcher::{reconcile_running_game_state, try_begin_launch_slot};
use dh_client_launcher::structs::launcher::ProcessLifecycle;
use dh_client_launcher::structs::state::{AppInstanceGuard, NamedMutexGuard, TrackedProcess};
use dh_win32::ProcessInfo;

static NEXT_MUTEX_SEQ: AtomicU64 = AtomicU64::new(1);

#[test]
fn target_game_exe_names_prioritizes_configured_exe_and_deduplicates_case_insensitively() {
    let default_names: Vec<String> =
        target_game_exe_names("./DreadHunger/Binaries/Win64/dreadhunger-win64-shipping.exe");
    assert_eq!(
        default_names,
        vec![
            "dreadhunger-win64-shipping.exe".to_string(),
            "DreadHunger.exe".to_string(),
        ]
    );

    let custom_names: Vec<String> = target_game_exe_names(r"D:\Games\CustomClient.exe");
    assert_eq!(
        custom_names,
        vec![
            "CustomClient.exe".to_string(),
            "DreadHunger-Win64-Shipping.exe".to_string(),
            "DreadHunger.exe".to_string(),
        ]
    );
}

#[test]
fn find_matching_process_in_matches_case_insensitively_with_priority_and_excludes_self() {
    let procs: Vec<ProcessInfo> = vec![
        ProcessInfo {
            pid: 0,
            parent_pid: 0,
            name: "DreadHunger-Win64-Shipping.exe".to_string(),
        },
        ProcessInfo {
            pid: 1001,
            parent_pid: 100,
            name: "DreadHunger.exe".to_string(),
        },
        ProcessInfo {
            pid: 2002,
            parent_pid: 1001,
            name: "DREADHUNGER-WIN64-SHIPPING.EXE".to_string(),
        },
        ProcessInfo {
            pid: 3003,
            parent_pid: 1,
            name: "notepad.exe".to_string(),
        },
    ];

    let exe_names: Vec<String> = target_game_exe_names("./DreadHunger-Win64-Shipping.exe");
    let first_match =
        find_matching_process_in(&procs, &exe_names, Some(9999)).expect("should match shipping");
    assert_eq!(first_match.pid, 2002);

    // Excluding 2002 falls back to DreadHunger.exe (1001).
    let fallback_match =
        find_matching_process_in(&procs, &exe_names, Some(2002)).expect("should match bootstrap");
    assert_eq!(fallback_match.pid, 1001);

    // Excluding both leaves no match (PID 0 is always ignored).
    let no_match = find_matching_process_in(&procs[..1], &exe_names, None);
    assert!(no_match.is_none());
}

#[test]
fn try_begin_launch_slot_blocks_concurrent_launch_and_alive_external_pid() {
    let tracked: Arc<Mutex<TrackedProcess>> = Arc::new(Mutex::new(TrackedProcess::default()));

    // First claim succeeds and marks state as injecting/launching.
    let (gen1, prev1) =
        try_begin_launch_slot(&tracked, "内蔵DLL (1件)").expect("first slot claim must succeed");
    assert_eq!(gen1, 1);
    assert!(prev1.is_none());

    // Second concurrent claim must fail immediately with ERR_LAUNCH_IN_PROGRESS.
    let err_in_flight: String = match try_begin_launch_slot(&tracked, "内蔵DLL (1件)") {
        Err(msg) => msg,
        Ok(_) => panic!("in-flight launch must be rejected"),
    };
    assert_eq!(err_in_flight, ERR_LAUNCH_IN_PROGRESS);

    // Simulate an already-running tracked external PID (current test process PID is guaranteed alive).
    let live_pid: u32 = std::process::id();
    assert!(is_pid_running(live_pid));
    {
        let mut guard = tracked.lock().expect("lock tracked");
        guard.launching = false;
        guard.external_pid = Some(live_pid);
    }

    let err_already_running: String = match try_begin_launch_slot(&tracked, "内蔵DLL (1件)") {
        Err(msg) => msg,
        Ok(_) => panic!("alive external pid must block second launch"),
    };
    assert!(err_already_running.contains(&live_pid.to_string()));
    assert!(err_already_running.contains("多重起動は許可されていません"));
}

#[test]
fn named_mutex_guard_blocks_second_holder_and_releases_cleanly_on_drop() {
    let seq: u64 = NEXT_MUTEX_SEQ.fetch_add(1, Ordering::Relaxed);
    let name: String = format!(
        r"Local\DH_ClientLauncher_JA_TestMutex_{}_{seq}",
        std::process::id()
    );

    let first_guard: NamedMutexGuard = try_acquire_named_mutex(&name, 50)
        .expect("acquire first mutex")
        .expect("first mutex must succeed");

    let second_attempt: Option<NamedMutexGuard> =
        try_acquire_named_mutex(&name, 30).expect("second attempt should return Ok(None)");
    assert!(second_attempt.is_none());

    drop(first_guard);

    let reacquired: NamedMutexGuard = try_acquire_named_mutex(&name, 50)
        .expect("reacquire after drop")
        .expect("mutex must be released cleanly on drop");
    drop(reacquired);
}

#[test]
fn reconcile_running_game_state_tracks_external_clears_exited_and_preserves_injecting() {
    let tracked: Arc<Mutex<TrackedProcess>> = Arc::new(Mutex::new(TrackedProcess::default()));

    // Detecting an external running game transitions Idle -> Running and marks watcher needed.
    let external: ProcessInfo = ProcessInfo {
        pid: 4242,
        parent_pid: 100,
        name: "DreadHunger-Win64-Shipping.exe".to_string(),
    };
    let outcome_detected =
        reconcile_running_game_state(&tracked, Some(&external)).expect("reconcile detected game");
    assert_eq!(outcome_detected.snapshot.status, ProcessLifecycle::Running);
    assert_eq!(outcome_detected.snapshot.pid, Some(4242));
    assert_eq!(outcome_detected.watch_target, Some((1, 4242)));
    assert!(outcome_detected.changed);

    // Repeating with the same running PID is a no-op (does not duplicate watcher).
    let outcome_repeat =
        reconcile_running_game_state(&tracked, Some(&external)).expect("reconcile same game");
    assert_eq!(outcome_repeat.snapshot.pid, Some(4242));
    assert!(outcome_repeat.watch_target.is_none());
    assert!(!outcome_repeat.changed);

    // When external process disappears, state transitions back to Idle cleanly.
    let outcome_cleared =
        reconcile_running_game_state(&tracked, None).expect("reconcile exited external pid");
    assert_eq!(outcome_cleared.snapshot.status, ProcessLifecycle::Idle);
    assert!(outcome_cleared.snapshot.pid.is_none());
    assert!(outcome_cleared.watch_target.is_none());
    assert!(outcome_cleared.changed);

    // Injecting state must never be overwritten while launch is in-flight.
    {
        let mut guard = tracked.lock().expect("lock tracked");
        guard.launching = true;
        guard.snapshot.status = ProcessLifecycle::Injecting;
    }
    let outcome_injecting =
        reconcile_running_game_state(&tracked, None).expect("reconcile while launching");
    assert_eq!(
        outcome_injecting.snapshot.status,
        ProcessLifecycle::Injecting
    );
    assert!(!outcome_injecting.changed);
}

#[test]
fn app_instance_guard_detects_duplicate_and_releases_on_drop() {
    let seq: u64 = NEXT_MUTEX_SEQ.fetch_add(1, Ordering::Relaxed);
    let name: String = format!(
        r"Local\DH_ClientLauncher_JA_AppTest_{}_{seq}",
        std::process::id()
    );

    let first: AppInstanceGuard = try_acquire_app_instance(&name)
        .expect("acquire first app instance")
        .expect("first app instance must succeed");

    let second: Option<AppInstanceGuard> =
        try_acquire_app_instance(&name).expect("duplicate check should return Ok(None)");
    assert!(second.is_none());

    drop(first);

    let reacquired: AppInstanceGuard = try_acquire_app_instance(&name)
        .expect("reacquire after drop")
        .expect("app instance mutex must be released on drop");
    drop(reacquired);

    assert!(!focus_existing_window(
        "DH_ClientLauncher_JA_Nonexistent_Window_Title_999999"
    ));
}
