pub mod cache;
pub mod consts;
pub mod error;
pub mod log_channel;
pub mod oep_inject;
pub mod prelude;
pub mod structs;

use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::os::windows::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use dh_win32::{spawn_suspended, JobObject, SpawnOptions, SpawnedProcess};
use tokio::sync::oneshot;
use tokio::time::{Interval, Sleep};

pub use cache::{default_cache_candidates, sweep_default_stale_run_dirs, sweep_stale_run_dirs};
pub use consts::cache::{FRIDA_GADGET_SHA256, FRIDA_GADGET_VERSION};
pub use consts::oep_inject::{MAX_DLL_PATH_UTF16_LEN, MAX_INJECT_DLLS};
pub use consts::prelude::WINDOWS_NATIVE_FUNCTION_PRELUDE;
pub use log_channel::start_log_server;
pub use oep_inject::ProcessInjector;
pub use prelude::{build_bundle_js, preprocess_mod_source};
pub use structs::{
    ActiveInjectionSession, BundleBuildParams, DllEventCallback, DllInjectionEvent, DllLoadResult,
    EventCallback, GadgetCache, InitHandshakeOutcome, InjectLaunchRequest, InjectedStubInfo,
    InjectorError, InjectorEvent, LogLevel, LogServerHandle, MultiDllStubHeader,
    MultiDllStubPayload, OepStubInjector, PatchMod, Result, RoundTempDir, StubProgress,
};

use crate::consts::log_channel::{
    DEFAULT_INIT_TIMEOUT_SECS, DEFAULT_PATCH_LOG_PATH, DLL_POLL_INTERVAL_MS,
    HANDSHAKE_POLL_INTERVAL_MS, PROCESS_EXIT_WAIT_MS,
};
use crate::structs::session::DllWaitConfig;

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

impl DllLoadResult {
    /// Returns `true` if `LoadLibraryW` returned a non-null `HMODULE` in the target process.
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.module_handle.is_some()
    }
}

impl InjectLaunchRequest {
    /// Creates a request with dedicated-server defaults:
    /// `create_console = true`, `attach_job_object = true`, `init_timeout = 30s`, no extra DLLs.
    #[must_use]
    pub fn new(exe_path: PathBuf, args: Vec<String>, mods: Vec<PatchMod>) -> Self {
        Self {
            exe_path,
            args,
            cwd: None,
            env_overrides: Vec::new(),
            dlls: Vec::new(),
            mods,
            tokens: HashMap::new(),
            patch_log_path: PathBuf::from(DEFAULT_PATCH_LOG_PATH),
            round: 1,
            init_timeout: Duration::from_secs(DEFAULT_INIT_TIMEOUT_SECS),
            cache_dir_override: None,
            create_console: true,
            attach_job_object: true,
            fail_on_dll_load_error: false,
            on_event: None,
            on_dll_event: None,
        }
    }

    /// Creates a request with GUI game-client defaults:
    /// `create_console = false` (no blank black console window),
    /// `attach_job_object = false` (closing the launcher does not kill a running game),
    /// optional ordered `dlls`, optional Frida Gadget `mods`, and `init_timeout = 30s`.
    #[must_use]
    pub fn for_gui_client(
        exe_path: PathBuf,
        args: Vec<String>,
        dlls: Vec<PathBuf>,
        mods: Vec<PatchMod>,
    ) -> Self {
        Self {
            exe_path,
            args,
            cwd: None,
            env_overrides: Vec::new(),
            dlls,
            mods,
            tokens: HashMap::new(),
            patch_log_path: PathBuf::from(DEFAULT_PATCH_LOG_PATH),
            round: 1,
            init_timeout: Duration::from_secs(DEFAULT_INIT_TIMEOUT_SECS),
            cache_dir_override: None,
            create_console: false,
            attach_job_object: false,
            fail_on_dll_load_error: false,
            on_event: None,
            on_dll_event: None,
        }
    }
}

async fn teardown_process_and_stage(
    process: Option<SpawnedProcess>,
    job: Option<JobObject>,
    round_dir: Option<RoundTempDir>,
    exit_code: u32,
    wait_for_exit: bool,
) {
    let _ = tokio::task::spawn_blocking(move || {
        if let Some(proc_handle) = process {
            if proc_handle.is_alive() {
                let _ = proc_handle.terminate(exit_code);
                if wait_for_exit {
                    let _ = proc_handle.wait_exit(PROCESS_EXIT_WAIT_MS);
                }
            }
            drop(proc_handle);
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
        self.process.as_ref().is_some_and(SpawnedProcess::is_alive)
    }

    #[must_use]
    pub fn round_dir(&self) -> Option<&Path> {
        self.round_dir.as_ref().map(RoundTempDir::dir)
    }

    #[must_use]
    pub fn stub_info(&self) -> &InjectedStubInfo {
        &self.stub_info
    }

    /// Returns `true` if an OEP bootstrap stub was installed (DLL and/or JS injection),
    /// or `false` for a plain launch.
    #[must_use]
    pub fn has_injected_stub(&self) -> bool {
        self.stub_info.code_page_addr != 0
    }

    /// Ordered `LoadLibraryW` results for all user-configured DLLs (`req.dlls`).
    #[must_use]
    pub fn dll_results(&self) -> &[DllLoadResult] {
        &self.dll_results
    }

    /// Non-blocking check for the target process exit code (`Ok(None)` if still running).
    pub fn try_exit_code(&self) -> Result<Option<u32>> {
        match self.process {
            Some(ref proc_handle) => Ok(proc_handle.wait_exit(0)?),
            None => Ok(None),
        }
    }

    /// Force-terminates the spawned process on demand (e.g., launcher stop button).
    pub fn terminate(&self, exit_code: u32) -> Result<()> {
        match self.process {
            Some(ref proc_handle) => Ok(proc_handle.terminate(exit_code)?),
            None => Ok(()),
        }
    }

    /// Reads the `HMODULE` returned by `LoadLibraryW` for the last injected DLL slot
    /// (`gadget.dll` when JS MODs are active, or the sole DLL for 1-DLL injection).
    #[must_use]
    pub fn loaded_module_handle(&self) -> Option<u64> {
        let proc_handle: &SpawnedProcess = self.process.as_ref()?;
        OepStubInjector.read_loaded_module_handle(proc_handle, &self.stub_info)
    }

    /// Gracefully shuts down the log server, terminates the process if still alive,
    /// releases the Job Object, and removes the per-round temporary directory.
    pub async fn cleanup(mut self) {
        let log_server: Option<LogServerHandle> = self.log_server.take();
        let process: Option<SpawnedProcess> = self.process.take();
        let job: Option<JobObject> = self.job.take();
        let round_dir: Option<RoundTempDir> = self.round_dir.take();
        if let Some(server) = log_server {
            server.shutdown().await;
        }
        teardown_process_and_stage(process, job, round_dir, 0, true).await;
    }

    /// Shuts down the launcher-side log server and closes process handles WITHOUT
    /// terminating the running process (intended for `attach_job_object == false` GUI clients).
    ///
    /// If the process is still running with `gadget.dll` mapped, per-round temp dir removal
    /// is deferred to `sweep_stale_run_dirs` on a future startup instead of stalling on file locks.
    pub async fn detach(mut self) {
        let log_server: Option<LogServerHandle> = self.log_server.take();
        let process: Option<SpawnedProcess> = self.process.take();
        let job: Option<JobObject> = self.job.take();
        let mut round_dir: Option<RoundTempDir> = self.round_dir.take();

        if let Some(server) = log_server {
            server.shutdown().await;
        }

        let keep_alive_detached: bool =
            job.is_none() && process.as_ref().is_some_and(SpawnedProcess::is_alive);
        if keep_alive_detached {
            if let Some(ref mut dir) = round_dir {
                dir.disarm_cleanup();
            }
        }
        drop(process);
        drop(job);
        drop(round_dir);
    }
}

impl Drop for ActiveInjectionSession {
    fn drop(&mut self) {
        // When launched detached (`attach_job_object == false`) and the game is still running,
        // avoid blocking Drop with retries against a locked `gadget.dll`; `sweep_stale_run_dirs`
        // reclaims the directory automatically on a subsequent launcher run.
        if self.job.is_none() && self.process.as_ref().is_some_and(SpawnedProcess::is_alive) {
            if let Some(ref mut dir) = self.round_dir {
                dir.disarm_cleanup();
            }
        }
    }
}

async fn abort_spawned_launch(
    process: SpawnedProcess,
    job: Option<JobObject>,
    round_dir: Option<RoundTempDir>,
    log_server: Option<LogServerHandle>,
    wait_for_exit: bool,
) {
    teardown_process_and_stage(Some(process), job, round_dir, 1, wait_for_exit).await;
    if let Some(server) = log_server {
        server.shutdown().await;
    }
}

fn normalize_clean_path(path: &Path) -> PathBuf {
    let mut normalized: PathBuf = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                let _: bool = normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    let normalized_str: Cow<'_, str> = normalized.to_string_lossy();
    if let Some(stripped) = normalized_str.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        normalized
    }
}

fn resolve_and_validate_user_dlls(
    raw_dlls: &[PathBuf],
    req_cwd: Option<&Path>,
    has_js: bool,
) -> Result<Vec<PathBuf>> {
    let total_dll_count: usize = raw_dlls.len().saturating_add(usize::from(has_js));
    if total_dll_count > MAX_INJECT_DLLS {
        return Err(InjectorError::TooManyDlls {
            count: total_dll_count,
            max: MAX_INJECT_DLLS,
        });
    }
    if raw_dlls.is_empty() {
        return Ok(Vec::new());
    }

    let current_dir: PathBuf = std::env::current_dir()?;
    let mut resolved: Vec<PathBuf> = Vec::with_capacity(raw_dlls.len());

    for raw_path in raw_dlls {
        if raw_path.as_os_str().is_empty() {
            return Err(InjectorError::Inject(
                "DLL path must not be empty".to_string(),
            ));
        }
        let raw_wide: Vec<u16> = raw_path.as_os_str().encode_wide().collect();
        if raw_wide.contains(&0) {
            return Err(InjectorError::Inject(
                "DLL path must not contain interior NUL characters".to_string(),
            ));
        }

        let candidate: PathBuf = if raw_path.is_absolute() {
            normalize_clean_path(raw_path)
        } else {
            let from_launcher_cwd: PathBuf = normalize_clean_path(&current_dir.join(raw_path));
            if from_launcher_cwd.is_file() {
                from_launcher_cwd
            } else if let Some(target_cwd) = req_cwd {
                let abs_target_cwd: PathBuf = if target_cwd.is_absolute() {
                    target_cwd.to_path_buf()
                } else {
                    current_dir.join(target_cwd)
                };
                normalize_clean_path(&abs_target_cwd.join(raw_path))
            } else {
                from_launcher_cwd
            }
        };

        if !candidate.is_file() {
            return Err(InjectorError::DllNotFound {
                path: raw_path.clone(),
            });
        }

        let utf16_len_with_nul: usize = candidate
            .as_os_str()
            .encode_wide()
            .count()
            .saturating_add(1);
        if utf16_len_with_nul > MAX_DLL_PATH_UTF16_LEN {
            return Err(InjectorError::PathTooLong {
                path: candidate,
                len: utf16_len_with_nul,
                max: MAX_DLL_PATH_UTF16_LEN,
            });
        }

        resolved.push(candidate);
    }

    Ok(resolved)
}

fn emit_new_dll_events(
    progress: &StubProgress,
    user_dlls: &[PathBuf],
    emitted_count: &mut usize,
    observed_handles: &mut [Option<u64>],
    on_dll_event: Option<&DllEventCallback>,
) {
    let target_done: usize = (progress.completed_count as usize).min(user_dlls.len());
    let total: usize = user_dlls.len();

    while *emitted_count < target_done {
        let idx: usize = *emitted_count;
        let raw_handle: u64 = progress.loaded_handles.get(idx).copied().unwrap_or(0);
        let handle_opt: Option<u64> = (raw_handle != 0).then_some(raw_handle);
        if let Some(slot) = observed_handles.get_mut(idx) {
            *slot = handle_opt;
        }
        if let Some(cb) = on_dll_event {
            let path: PathBuf = user_dlls[idx].clone();
            match handle_opt {
                Some(module_handle) => cb(DllInjectionEvent::Loaded {
                    index: idx,
                    total,
                    path,
                    module_handle,
                }),
                None => cb(DllInjectionEvent::Failed {
                    index: idx,
                    total,
                    path,
                }),
            }
        }
        *emitted_count = idx.saturating_add(1);
    }
}

fn build_dll_results(
    user_dlls: &[PathBuf],
    observed_handles: &[Option<u64>],
    fail_on_dll_load_error: bool,
) -> Result<Vec<DllLoadResult>> {
    let mut results: Vec<DllLoadResult> = Vec::with_capacity(user_dlls.len());
    for (idx, path) in user_dlls.iter().enumerate() {
        let module_handle: Option<u64> = observed_handles.get(idx).copied().flatten();
        if fail_on_dll_load_error && module_handle.is_none() {
            return Err(InjectorError::DllLoadFailed { path: path.clone() });
        }
        results.push(DllLoadResult {
            path: path.clone(),
            module_handle,
        });
    }
    Ok(results)
}

/// Spawns the target process suspended, attaches any configured DLLs and/or Frida Gadget JS MODs
/// via [`OepStubInjector`], resumes the process, and waits for completion/handshake.
pub async fn spawn_and_inject(req: InjectLaunchRequest) -> Result<ActiveInjectionSession> {
    let injector: OepStubInjector = OepStubInjector;
    spawn_and_inject_with_cancel(&injector, req, std::future::pending()).await
}

/// Spawns the target process suspended using a custom [`ProcessInjector`], supporting all 4 modes:
/// 1. Plain launch (`dlls.is_empty() && mods.is_empty()`) — zero Gadget or OEP overhead.
/// 2. DLL-only (`!dlls.is_empty() && mods.is_empty()`) — zero Frida Gadget overhead; injects DLLs
///    at OEP, streams `DllInjectionEvent`, and waits for OEP stub completion.
/// 3. JS-only (`dlls.is_empty() && !mods.is_empty()`) — Frida Gadget early-injection (existing behavior).
/// 4. DLLs + JS (`!dlls.is_empty() && !mods.is_empty()`) — single atomic pre-OEP bootstrap loading
///    user DLLs in order followed by `gadget.dll`, streaming both `DllInjectionEvent` and `InjectorEvent`.
///
/// On cancellation, error, or timeout before session handoff, the spawned process is immediately
/// terminated and any temporary staging directory is cleaned up.
pub async fn spawn_and_inject_with_cancel<I, F>(
    injector: &I,
    req: InjectLaunchRequest,
    cancel: F,
) -> Result<ActiveInjectionSession>
where
    I: ProcessInjector,
    F: Future<Output = ()>,
{
    let has_js: bool = !req.mods.is_empty();
    let resolved_user_dlls: Vec<PathBuf> =
        resolve_and_validate_user_dlls(&req.dlls, req.cwd.as_deref(), has_js)?;
    let has_user_dlls: bool = !resolved_user_dlls.is_empty();
    let spawn_options: SpawnOptions = SpawnOptions {
        create_console: req.create_console,
        env_overrides: req.env_overrides,
    };

    // Mode 1: Plain launch (0 DLLs, 0 JS) — zero Frida Gadget cache, log server, or OEP stub overhead.
    if !has_user_dlls && !has_js {
        let job: Option<JobObject> = if req.attach_job_object {
            Some(JobObject::new_kill_on_close()?)
        } else {
            None
        };
        let process: SpawnedProcess =
            spawn_suspended(&req.exe_path, &req.args, req.cwd.as_deref(), &spawn_options)?;
        let pid: u32 = process.pid();

        if let Some(ref job_obj) = job {
            if let Err(err) = job_obj.assign_process(&process) {
                abort_spawned_launch(process, job, None, None, false).await;
                return Err(err.into());
            }
        }
        if let Err(err) = process.resume() {
            abort_spawned_launch(process, job, None, None, false).await;
            return Err(err.into());
        }

        return Ok(ActiveInjectionSession {
            pid,
            process: Some(process),
            log_server: None,
            job,
            round_dir: None,
            stub_info: InjectedStubInfo::empty(),
            dll_results: Vec::new(),
        });
    }

    // Mode 2: DLL-only OEP injection (N DLLs, 0 JS) — zero Frida Gadget overhead.
    if has_user_dlls && !has_js {
        let job: Option<JobObject> = if req.attach_job_object {
            Some(JobObject::new_kill_on_close()?)
        } else {
            None
        };
        let process: SpawnedProcess =
            spawn_suspended(&req.exe_path, &req.args, req.cwd.as_deref(), &spawn_options)?;
        let pid: u32 = process.pid();

        if let Some(ref job_obj) = job {
            if let Err(err) = job_obj.assign_process(&process) {
                abort_spawned_launch(process, job, None, None, false).await;
                return Err(err.into());
            }
        }

        let stub_info: InjectedStubInfo =
            match injector.inject_dlls_suspended(&process, &resolved_user_dlls) {
                Ok(info) => info,
                Err(err) => {
                    abort_spawned_launch(process, job, None, None, false).await;
                    return Err(err);
                }
            };

        if let Err(err) = process.resume() {
            abort_spawned_launch(process, job, None, None, false).await;
            return Err(err.into());
        }

        let wait_cfg: DllWaitConfig<'_> = DllWaitConfig {
            user_dlls: &resolved_user_dlls,
            init_timeout: req.init_timeout,
            fail_on_dll_load_error: req.fail_on_dll_load_error,
            on_dll_event: req.on_dll_event.as_ref(),
        };
        let wait_result: Result<Vec<DllLoadResult>> =
            wait_for_dll_only_completion(injector, &process, &stub_info, &wait_cfg, cancel).await;

        return match wait_result {
            Ok(dll_results) => Ok(ActiveInjectionSession {
                pid,
                process: Some(process),
                log_server: None,
                job,
                round_dir: None,
                stub_info,
                dll_results,
            }),
            Err(err) => {
                abort_spawned_launch(process, job, None, None, true).await;
                Err(err)
            }
        };
    }

    // Mode 3 & 4: JS-only OR DLLs + JS together in ONE atomic pre-OEP bootstrap.
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
    let job: Option<JobObject> = if req.attach_job_object {
        Some(JobObject::new_kill_on_close()?)
    } else {
        None
    };

    let mut all_dlls: Vec<PathBuf> = Vec::with_capacity(resolved_user_dlls.len() + 1);
    all_dlls.extend(resolved_user_dlls.iter().cloned());
    all_dlls.push(round_dir.gadget_dll_path().to_path_buf());

    let process: SpawnedProcess =
        spawn_suspended(&req.exe_path, &req.args, req.cwd.as_deref(), &spawn_options)?;
    let pid: u32 = process.pid();

    if let Some(ref job_obj) = job {
        if let Err(err) = job_obj.assign_process(&process) {
            abort_spawned_launch(process, job, Some(round_dir), Some(log_server), false).await;
            return Err(err.into());
        }
    }

    let stub_info: InjectedStubInfo = match injector.inject_dlls_suspended(&process, &all_dlls) {
        Ok(info) => info,
        Err(err) => {
            abort_spawned_launch(process, job, Some(round_dir), Some(log_server), false).await;
            return Err(err);
        }
    };

    if let Err(err) = process.resume() {
        abort_spawned_launch(process, job, Some(round_dir), Some(log_server), false).await;
        return Err(err.into());
    }

    let wait_cfg: DllWaitConfig<'_> = DllWaitConfig {
        user_dlls: &resolved_user_dlls,
        init_timeout: req.init_timeout,
        fail_on_dll_load_error: req.fail_on_dll_load_error,
        on_dll_event: req.on_dll_event.as_ref(),
    };
    let wait_result: Result<Vec<DllLoadResult>> =
        wait_for_js_and_dll_handshake(injector, &process, &stub_info, &wait_cfg, init_rx, cancel)
            .await;

    match wait_result {
        Ok(dll_results) => Ok(ActiveInjectionSession {
            pid,
            process: Some(process),
            log_server: Some(log_server),
            job,
            round_dir: Some(round_dir),
            stub_info,
            dll_results,
        }),
        Err(err) => {
            abort_spawned_launch(process, job, Some(round_dir), Some(log_server), true).await;
            Err(err)
        }
    }
}

async fn wait_for_dll_only_completion<I, F>(
    injector: &I,
    process: &SpawnedProcess,
    stub_info: &InjectedStubInfo,
    cfg: &DllWaitConfig<'_>,
    cancel: F,
) -> Result<Vec<DllLoadResult>>
where
    I: ProcessInjector,
    F: Future<Output = ()>,
{
    let deadline: Sleep = tokio::time::sleep(cfg.init_timeout);
    tokio::pin!(deadline);
    tokio::pin!(cancel);

    let mut poll: Interval = tokio::time::interval(Duration::from_millis(DLL_POLL_INTERVAL_MS));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut emitted_count: usize = 0;
    let mut observed_handles: Vec<Option<u64>> = vec![None; cfg.user_dlls.len()];

    loop {
        tokio::select! {
            () = &mut cancel => {
                return Err(InjectorError::Cancelled);
            }
            () = &mut deadline => {
                let detail: String = format!(
                    "OEP DLL bootstrap did not finish ({emitted_count}/{} DLLs completed)",
                    cfg.user_dlls.len()
                );
                return Err(InjectorError::InitTimeout {
                    timeout_secs: cfg.init_timeout.as_secs(),
                    detail,
                });
            }
            _ = poll.tick() => {
                if let Some(progress) = injector.read_stub_progress(process, stub_info) {
                    emit_new_dll_events(
                        &progress,
                        cfg.user_dlls,
                        &mut emitted_count,
                        &mut observed_handles,
                        cfg.on_dll_event,
                    );
                    if progress.stub_finished {
                        return build_dll_results(
                            cfg.user_dlls,
                            &observed_handles,
                            cfg.fail_on_dll_load_error,
                        );
                    }
                }
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

async fn wait_for_js_and_dll_handshake<I, F>(
    injector: &I,
    process: &SpawnedProcess,
    stub_info: &InjectedStubInfo,
    cfg: &DllWaitConfig<'_>,
    mut init_rx: oneshot::Receiver<InitHandshakeOutcome>,
    cancel: F,
) -> Result<Vec<DllLoadResult>>
where
    I: ProcessInjector,
    F: Future<Output = ()>,
{
    let deadline: Sleep = tokio::time::sleep(cfg.init_timeout);
    tokio::pin!(deadline);
    tokio::pin!(cancel);

    let mut poll: Interval =
        tokio::time::interval(Duration::from_millis(HANDSHAKE_POLL_INTERVAL_MS));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut emitted_count: usize = 0;
    let mut observed_handles: Vec<Option<u64>> = vec![None; cfg.user_dlls.len()];

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
                    timeout_secs: cfg.init_timeout.as_secs(),
                    detail,
                });
            }
            res = &mut init_rx => {
                if !cfg.user_dlls.is_empty() {
                    if let Some(progress) = injector.read_stub_progress(process, stub_info) {
                        emit_new_dll_events(
                            &progress,
                            cfg.user_dlls,
                            &mut emitted_count,
                            &mut observed_handles,
                            cfg.on_dll_event,
                        );
                    }
                }
                match res {
                    Ok(InitHandshakeOutcome::Ok) => {
                        return build_dll_results(
                            cfg.user_dlls,
                            &observed_handles,
                            cfg.fail_on_dll_load_error,
                        );
                    }
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
            _ = poll.tick() => {
                if let Some(progress) = injector.read_stub_progress(process, stub_info) {
                    if !cfg.user_dlls.is_empty() {
                        emit_new_dll_events(
                            &progress,
                            cfg.user_dlls,
                            &mut emitted_count,
                            &mut observed_handles,
                            cfg.on_dll_event,
                        );
                    }
                    // Fast-fail if the OEP stub already finished and `gadget.dll` (last slot) returned NULL.
                    if progress.stub_finished {
                        let gadget_handle: u64 = progress.loaded_handles.last().copied().unwrap_or(0);
                        if gadget_handle == 0 {
                            return Err(InjectorError::Inject(
                                "LoadLibraryW(gadget.dll) returned NULL".to_string(),
                            ));
                        }
                    }
                }
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
