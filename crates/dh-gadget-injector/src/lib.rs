pub mod cache;
pub mod consts;
pub mod error;
pub mod log_channel;
pub mod oep_inject;
pub mod prelude;
pub mod structs;

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dh_win32::{spawn_suspended_console, JobObject, SpawnedProcess};
use tokio::sync::oneshot;
use tokio::time::{Interval, Sleep};

pub use cache::{default_cache_candidates, sweep_default_stale_run_dirs, sweep_stale_run_dirs};
pub use consts::cache::{FRIDA_GADGET_SHA256, FRIDA_GADGET_VERSION};
pub use consts::prelude::WINDOWS_NATIVE_FUNCTION_PRELUDE;
pub use log_channel::start_log_server;
pub use oep_inject::ProcessInjector;
pub use prelude::{build_bundle_js, preprocess_mod_source};
pub use structs::{
    ActiveInjectionSession, BundleBuildParams, EventCallback, GadgetCache, InitHandshakeOutcome,
    InjectLaunchRequest, InjectedStubInfo, InjectorError, InjectorEvent, LogLevel, LogServerHandle,
    OepStubInjector, PatchMod, Result, RoundTempDir,
};

use crate::consts::log_channel::{
    DEFAULT_INIT_TIMEOUT_SECS, DEFAULT_PATCH_LOG_PATH, EXIT_POLL_INTERVAL_MS, PROCESS_EXIT_WAIT_MS,
};

impl LogLevel {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

impl InjectLaunchRequest {
    /// Creates a request with standard defaults (`init_timeout = 30s`, default cache directory).
    #[must_use]
    pub fn new(exe_path: PathBuf, args: Vec<String>, mods: Vec<PatchMod>) -> Self {
        Self {
            exe_path,
            args,
            cwd: None,
            mods,
            tokens: HashMap::new(),
            patch_log_path: PathBuf::from(DEFAULT_PATCH_LOG_PATH),
            round: 1,
            init_timeout: Duration::from_secs(DEFAULT_INIT_TIMEOUT_SECS),
            cache_dir_override: None,
            on_event: None,
        }
    }
}

async fn teardown_process_and_stage(
    process: SpawnedProcess,
    job: Option<JobObject>,
    round_dir: Option<RoundTempDir>,
    exit_code: u32,
    wait_for_exit: bool,
) {
    let _ = tokio::task::spawn_blocking(move || {
        if process.is_alive() {
            let _ = process.terminate(exit_code);
            if wait_for_exit {
                let _ = process.wait_exit(PROCESS_EXIT_WAIT_MS);
            }
        }
        // Explicitly drop JobObject before cleaning the staging directory so Windows
        // finishes unmapping `gadget.dll` prior to directory removal.
        drop(job);
        if let Some(mut dir) = round_dir {
            dir.cleanup();
        }
    })
    .await;
}

impl ActiveInjectionSession {
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.pid
    }

    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.process.is_alive()
    }

    #[must_use]
    pub fn round_dir(&self) -> Option<&Path> {
        self.round_dir.as_ref().map(RoundTempDir::dir)
    }

    #[must_use]
    pub fn stub_info(&self) -> &InjectedStubInfo {
        &self.stub_info
    }

    /// Reads the `HMODULE` returned by `LoadLibraryW(gadget.dll)` in the remote process, if available.
    #[must_use]
    pub fn loaded_module_handle(&self) -> Option<u64> {
        OepStubInjector.read_loaded_module_handle(&self.process, &self.stub_info)
    }

    /// Gracefully shuts down the log server, terminates the process if still alive,
    /// releases the Job Object, and removes the per-round temporary directory.
    pub async fn cleanup(self) {
        let Self {
            process,
            log_server,
            job,
            round_dir,
            ..
        } = self;
        if let Some(server) = log_server {
            server.shutdown().await;
        }
        teardown_process_and_stage(process, job, round_dir, 0, true).await;
    }
}

async fn abort_spawned_launch(
    process: SpawnedProcess,
    job: JobObject,
    round_dir: RoundTempDir,
    log_server: LogServerHandle,
    wait_for_exit: bool,
) {
    teardown_process_and_stage(process, Some(job), Some(round_dir), 1, wait_for_exit).await;
    log_server.shutdown().await;
}

/// Spawns the target process suspended, attaches Frida Gadget via [`OepStubInjector`],
/// resumes the process, and waits up to `req.init_timeout` for `INIT_OK`.
pub async fn spawn_and_inject(req: InjectLaunchRequest) -> Result<ActiveInjectionSession> {
    let injector: OepStubInjector = OepStubInjector;
    spawn_and_inject_with_cancel(&injector, req, std::future::pending()).await
}

/// Spawns the target process suspended using a custom [`ProcessInjector`], racing against
/// a cancellation future (e.g., `cancel_token.cancelled()`). On cancellation, error, or
/// timeout, the spawned process is immediately terminated and the per-round staging
/// directory is cleaned up.
pub async fn spawn_and_inject_with_cancel<I, F>(
    injector: &I,
    req: InjectLaunchRequest,
    cancel: F,
) -> Result<ActiveInjectionSession>
where
    I: ProcessInjector,
    F: Future<Output = ()>,
{
    let cache: GadgetCache = GadgetCache::open(req.cache_dir_override.as_deref())?;
    let (log_server, init_rx): (LogServerHandle, oneshot::Receiver<InitHandshakeOutcome>) =
        start_log_server(req.on_event.clone()).await?;

    let patch_log_abs: PathBuf = if req.patch_log_path.is_absolute() {
        req.patch_log_path.clone()
    } else if let Some(ref cwd) = req.cwd {
        cwd.join(&req.patch_log_path)
    } else {
        std::env::current_dir()?.join(&req.patch_log_path)
    };

    let bundle_js: String = build_bundle_js(&BundleBuildParams {
        mods: &req.mods,
        tokens: &req.tokens,
        log_port: log_server.port(),
        session_nonce: log_server.session_nonce(),
        patch_log_path: &patch_log_abs,
    })?;

    // Declaration order ensures reverse drop order kills `job` before cleaning `round_dir`
    // if this future is externally dropped while awaiting.
    let round_dir: RoundTempDir = cache.prepare_round_dir(req.round, &bundle_js)?;
    let job: JobObject = JobObject::new_kill_on_close()?;
    let process: SpawnedProcess =
        spawn_suspended_console(&req.exe_path, &req.args, req.cwd.as_deref())?;
    let pid: u32 = process.pid();

    if let Err(err) = job.assign_process(&process) {
        abort_spawned_launch(process, job, round_dir, log_server, false).await;
        return Err(err.into());
    }

    let stub_info: InjectedStubInfo =
        match injector.inject_suspended(&process, round_dir.gadget_dll_path()) {
            Ok(info) => info,
            Err(err) => {
                abort_spawned_launch(process, job, round_dir, log_server, false).await;
                return Err(err);
            }
        };

    if let Err(err) = process.resume() {
        abort_spawned_launch(process, job, round_dir, log_server, false).await;
        return Err(err.into());
    }

    let wait_result: Result<()> = wait_for_handshake(
        injector,
        &process,
        &stub_info,
        init_rx,
        req.init_timeout,
        cancel,
    )
    .await;

    match wait_result {
        Ok(()) => Ok(ActiveInjectionSession {
            pid,
            process,
            log_server: Some(log_server),
            job: Some(job),
            round_dir: Some(round_dir),
            stub_info,
        }),
        Err(err) => {
            abort_spawned_launch(process, job, round_dir, log_server, true).await;
            Err(err)
        }
    }
}

async fn wait_for_handshake<I, F>(
    injector: &I,
    process: &SpawnedProcess,
    stub_info: &InjectedStubInfo,
    mut init_rx: oneshot::Receiver<InitHandshakeOutcome>,
    init_timeout: Duration,
    cancel: F,
) -> Result<()>
where
    I: ProcessInjector,
    F: Future<Output = ()>,
{
    let deadline: Sleep = tokio::time::sleep(init_timeout);
    tokio::pin!(deadline);
    tokio::pin!(cancel);

    let mut exit_poll: Interval =
        tokio::time::interval(Duration::from_millis(EXIT_POLL_INTERVAL_MS));
    exit_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            () = &mut cancel => {
                return Err(InjectorError::Cancelled);
            }
            () = &mut deadline => {
                let module_handle: Option<u64> = injector.read_loaded_module_handle(process, stub_info);
                let detail: String = match module_handle {
                    Some(0) => "LoadLibraryW returned NULL or has not completed".to_string(),
                    Some(h) => format!("frida-gadget loaded at {h:#x}, but INIT_OK was not received"),
                    None => "unable to inspect remote bootstrap state".to_string(),
                };
                return Err(InjectorError::InitTimeout {
                    timeout_secs: init_timeout.as_secs(),
                    detail,
                });
            }
            res = &mut init_rx => {
                match res {
                    Ok(InitHandshakeOutcome::Ok) => return Ok(()),
                    Ok(InitHandshakeOutcome::Error { file_name, message }) => {
                        return Err(InjectorError::ModInit { file_name, message });
                    }
                    Err(_) => {
                        return Err(InjectorError::Inject(
                            "loopback log server closed before INIT_OK".to_string(),
                        ));
                    }
                }
            }
            _ = exit_poll.tick() => {
                if !process.is_alive() {
                    let exit_code: Option<u32> = process.wait_exit(0).ok().flatten();
                    return Err(InjectorError::PrematureExit {
                        pid: process.pid(),
                        exit_code,
                    });
                }
            }
        }
    }
}
