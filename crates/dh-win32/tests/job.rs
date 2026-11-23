use std::path::Path;
use std::time::{Duration, Instant};

use dh_win32::job::JobObject;
use dh_win32::process::{is_process_alive, spawn_suspended_console};

#[test]
fn job_object_kills_assigned_process_on_drop() {
    let cmd = Path::new(r"C:\Windows\System32\cmd.exe");
    let args = vec!["/C".to_string(), "ping -n 30 127.0.0.1 >NUL".to_string()];
    let proc = spawn_suspended_console(cmd, &args, None).expect("spawn cmd.exe suspended");
    let pid = proc.pid();

    let job = JobObject::new_kill_on_close().expect("create job object");
    job.assign_process(&proc).expect("assign process to job");
    proc.resume().expect("resume process");
    assert!(is_process_alive(pid));

    // Dropping the JobObject must immediately kill the assigned child process.
    drop(job);

    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if !is_process_alive(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let _ = proc.terminate(1);
    panic!("process {pid} remained alive after JobObject drop");
}
