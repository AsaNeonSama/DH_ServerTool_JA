import { resolve } from "path";
import frida, { LogLevel, MessageType, Stdio } from "frida";
import type { Message, Script, Session, SessionDetachReason } from "frida";
import {
  C,
  FRIDA_ATTACH_MAX_RETRIES,
  FRIDA_ATTACH_RETRY_DELAY_MS,
  FRIDA_CLEANUP_TIMEOUT_MS,
  FRIDA_RPC_TIMEOUT_MS,
  UE_PROJECT_NAME,
  WINDOWS_CONSOLE_BOOTSTRAP_SCRIPT,
  WINDOWS_NATIVE_FUNCTION_PRELUDE,
} from "../constants";
import { SERVER_MESSAGES } from "../messages";
import { println } from "../terminal";
import type {
  ActiveInjectionSession,
  PatchScript,
  SpawnAndInjectOptions,
} from "../types";
import {
  bindProcessToLauncherJob,
  isProcessAlive,
  terminateProcessByPid,
} from "../win32";
import { appendPatchRuntimeLog, resetPatchRuntimeLog } from "./patchLogger";
import type { PatchLogLevel } from "./patchLogger";

interface TimedOpOptions {
  readonly timeoutMs: number;
  readonly label: string;
  readonly signal?: AbortSignal;
}

function createAbortError(message: string): Error {
  const err: Error = new Error(message);
  err.name = "AbortError";
  return err;
}

function toError(err: unknown, fallback: string): Error {
  if (err instanceof Error) {
    return err;
  }
  if (typeof err === "string" && err.length > 0) {
    return new Error(err);
  }
  return new Error(fallback);
}

function mapFridaLogLevel(level: LogLevel): PatchLogLevel {
  if (level === LogLevel.Error) {
    return "ERROR";
  }
  if (level === LogLevel.Warning) {
    return "WARN";
  }
  return "INFO";
}

function runWithTimeout<T>(
  promise: Promise<T>,
  options: TimedOpOptions
): Promise<T> {
  const { timeoutMs, label, signal }: TimedOpOptions = options;
  if (signal?.aborted) {
    // Suppress potential late rejection from the abandoned operation
    promise.catch((): void => undefined);
    return Promise.reject(createAbortError(SERVER_MESSAGES.opAborted(label)));
  }

  const guarded: Promise<T> = new Promise(
    (resolvePromise: (value: T) => void, rejectPromise: (err: Error) => void): void => {
      let settled: boolean = false;

      const cleanup = (): void => {
        clearTimeout(timer);
        if (signal) {
          signal.removeEventListener("abort", onAbort);
        }
      };

      const onAbort = (): void => {
        if (settled) return;
        settled = true;
        cleanup();
        rejectPromise(createAbortError(SERVER_MESSAGES.opAborted(label)));
      };

      const timer: ReturnType<typeof setTimeout> = setTimeout((): void => {
        if (settled) return;
        settled = true;
        cleanup();
        rejectPromise(new Error(SERVER_MESSAGES.opTimedOut(label, timeoutMs)));
      }, timeoutMs);

      if (signal) {
        signal.addEventListener("abort", onAbort, { once: true });
      }

      promise.then(
        (value: T): void => {
          if (settled) return;
          settled = true;
          cleanup();
          resolvePromise(value);
        },
        (err: unknown): void => {
          if (settled) return;
          settled = true;
          cleanup();
          rejectPromise(toError(err, SERVER_MESSAGES.opFailed(label)));
        }
      );
    }
  );
  return guarded;
}

function sleepDelay(ms: number, signal?: AbortSignal): Promise<void> {
  if (signal?.aborted) {
    return Promise.reject(createAbortError(SERVER_MESSAGES.injectionAborted));
  }
  const waiter: Promise<void> = new Promise(
    (resolveDelay: () => void, rejectDelay: (err: Error) => void): void => {
      const onAbort = (): void => {
        clearTimeout(timer);
        rejectDelay(createAbortError(SERVER_MESSAGES.injectionAborted));
      };
      const timer: ReturnType<typeof setTimeout> = setTimeout((): void => {
        signal?.removeEventListener("abort", onAbort);
        resolveDelay();
      }, ms);
      signal?.addEventListener("abort", onAbort, { once: true });
    }
  );
  return waiter;
}

function safeStringifyPayload(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

async function attachWithRetry(
  pid: number,
  signal?: AbortSignal
): Promise<Session> {
  let lastError: unknown = null;
  for (let attempt: number = 1; attempt <= FRIDA_ATTACH_MAX_RETRIES; attempt++) {
    if (signal?.aborted) {
      throw createAbortError(SERVER_MESSAGES.attachAborted);
    }
    try {
      return await runWithTimeout(frida.attach(pid), {
        timeoutMs: FRIDA_RPC_TIMEOUT_MS,
        label: SERVER_MESSAGES.attachLabel(pid),
        signal,
      });
    } catch (err: unknown) {
      lastError = err;
      if (
        signal?.aborted ||
        !isProcessAlive(pid) ||
        attempt >= FRIDA_ATTACH_MAX_RETRIES
      ) {
        throw err;
      }
      await sleepDelay(FRIDA_ATTACH_RETRY_DELAY_MS * attempt, signal);
    }
  }
  throw toError(lastError, SERVER_MESSAGES.attachFailedFallback);
}

/**
 * Spawns DreadHungerServer-Win64-Shipping.exe directly in a suspended state
 * via Frida, injects all selected MOD scripts at 0s (before UE4 PreInit and
 * UObject::Serialize run), and then resumes the process.
 *
 * All Frida JS patch runtime logs (`send(...)`, `console.*`, and JS errors) are
 * written to `./patch_runtime.log` (overwritten every match round) and never
 * printed to Tab 2.
 */
export async function spawnAndInjectServer(
  options: SpawnAndInjectOptions
): Promise<ActiveInjectionSession> {
  const {
    shippingExecutable,
    ueMapArgument,
    port,
    patches,
    signal,
    onDetached,
  }: SpawnAndInjectOptions = options;

  if (signal?.aborted) {
    throw createAbortError(SERVER_MESSAGES.launchAborted);
  }

  // Overwrite per-match patch runtime log file in the WindowsServer root directory
  resetPatchRuntimeLog({ port, patches });

  const resolvedExe: string = resolve(shippingExecutable);
  const argv: string[] = [
    resolvedExe,
    UE_PROJECT_NAME,
    ueMapArgument,
    "-log",
    `-port=${port}`,
  ];

  println(
    `${C.LIGHT_CYAN} * ${SERVER_MESSAGES.spawningSuspended(shippingExecutable, port)}`
  );

  const pid: number = await runWithTimeout(
    frida.spawn(argv, {
      cwd: process.cwd(),
      stdio: Stdio.Inherit,
    }),
    {
      timeoutMs: FRIDA_RPC_TIMEOUT_MS,
      label: SERVER_MESSAGES.spawnLabel,
      signal,
    }
  );

  bindProcessToLauncherJob(pid);

  println(
    `${C.LIGHT_GREEN} * ${SERVER_MESSAGES.spawnedProcessPrefix} ${C.LIGHT_WHITE}${pid}${C.LIGHT_GREEN}${SERVER_MESSAGES.spawnedProcessSuffix}`
  );

  let session: Session | null = null;
  let detached: boolean = false;
  let reasonStr: string | null = null;
  const scripts: Script[] = [];

  try {
    session = await attachWithRetry(pid, signal);

    session.detached.connect((reason: SessionDetachReason): void => {
      detached = true;
      reasonStr = String(reason);
      onDetached?.(reasonStr);
    });

    // Clear inherited STARTF_USESTDHANDLES so UE4's -log AllocConsole() writes to the new server console
    let consoleBootstrap: Script | null = null;
    try {
      consoleBootstrap = await runWithTimeout(
        session.createScript(WINDOWS_CONSOLE_BOOTSTRAP_SCRIPT, {
          name: "runtime:windows_console_bootstrap",
        }),
        {
          timeoutMs: FRIDA_RPC_TIMEOUT_MS,
          label: SERVER_MESSAGES.consoleBootstrapCreateLabel,
          signal,
        }
      );
      consoleBootstrap.logHandler = (level: LogLevel, text: string): void => {
        appendPatchRuntimeLog({
          level: mapFridaLogLevel(level),
          fileName: "windows_console_bootstrap",
          message: text,
        });
      };
      await runWithTimeout(consoleBootstrap.load(), {
        timeoutMs: FRIDA_RPC_TIMEOUT_MS,
        label: SERVER_MESSAGES.consoleBootstrapLoadLabel,
        signal,
      });
    } finally {
      if (consoleBootstrap && !detached) {
        try {
          await runWithTimeout(consoleBootstrap.unload(), {
            timeoutMs: FRIDA_CLEANUP_TIMEOUT_MS,
            label: SERVER_MESSAGES.consoleBootstrapUnloadLabel,
          });
        } catch {
          // Non-fatal cleanup failure
        }
      }
    }

    for (let i: number = 0; i < patches.length; i++) {
      const patch: PatchScript | undefined = patches[i];
      if (!patch) continue;

      if (signal?.aborted) {
        throw createAbortError(SERVER_MESSAGES.patchInjectionAborted);
      }

      if (detached) {
        throw new Error(
          SERVER_MESSAGES.processDetachedDuringInjection(reasonStr ?? "detached")
        );
      }

      println(
        `${C.CYAN}   ${SERVER_MESSAGES.injectingPatchPrefix(i + 1, patches.length)} ${C.LIGHT_WHITE}${patch.label} ${C.LIGHT_BLACK}(${patch.fileName})`
      );

      const cleanSource: string =
        WINDOWS_NATIVE_FUNCTION_PRELUDE + patch.source.replace(/^\uFEFF/, "");
      const script: Script = await runWithTimeout(
        session.createScript(cleanSource, {
          name: patch.id,
        }),
        {
          timeoutMs: FRIDA_RPC_TIMEOUT_MS,
          label: SERVER_MESSAGES.scriptCreateLabel(patch.fileName),
          signal,
        }
      );

      const loadState: { loading: boolean; error: string | null } = {
        loading: true,
        error: null,
      };

      script.logHandler = (level: LogLevel, text: string): void => {
        appendPatchRuntimeLog({
          level: mapFridaLogLevel(level),
          fileName: patch.fileName,
          message: text,
        });
      };

      script.message.connect((message: Message): void => {
        if (message.type === MessageType.Error) {
          const desc: string =
            "description" in message && typeof message.description === "string"
              ? message.description
              : safeStringifyPayload(message);
          const stack: string =
            "stack" in message && typeof message.stack === "string"
              ? `\n${message.stack}`
              : "";
          if (loadState.loading && loadState.error === null) {
            loadState.error = desc;
          }
          appendPatchRuntimeLog({
            level: "ERROR",
            fileName: patch.fileName,
            message: `${desc}${stack}`,
          });
        } else if (message.type === MessageType.Send) {
          const text: string = safeStringifyPayload(message.payload);
          appendPatchRuntimeLog({
            level: "INFO",
            fileName: patch.fileName,
            message: text,
          });
        }
      });

      await runWithTimeout(script.load(), {
        timeoutMs: FRIDA_RPC_TIMEOUT_MS,
        label: SERVER_MESSAGES.scriptLoadLabel(patch.fileName),
        signal,
      });
      loadState.loading = false;
      scripts.push(script);

      if (loadState.error !== null) {
        throw new Error(
          SERVER_MESSAGES.patchInitError(patch.fileName, loadState.error)
        );
      }

      println(
        `${C.LIGHT_GREEN}   ${SERVER_MESSAGES.injectedPatchDone(i + 1, patches.length, patch.label)}`
      );
    }

    if (signal?.aborted) {
      throw createAbortError(SERVER_MESSAGES.resumeAborted);
    }

    await runWithTimeout(frida.resume(pid), {
      timeoutMs: FRIDA_RPC_TIMEOUT_MS,
      label: SERVER_MESSAGES.resumeLabel(pid),
      signal,
    });
    println(
      `${C.LIGHT_GREEN} * ${SERVER_MESSAGES.allPatchesInjectedResumed(patches.length)}`
    );
  } catch (err: unknown) {
    for (const s of scripts) {
      try {
        if (!detached) {
          await runWithTimeout(s.unload(), {
            timeoutMs: FRIDA_CLEANUP_TIMEOUT_MS,
            label: SERVER_MESSAGES.scriptUnloadLabel,
          });
        }
      } catch {
        // Ignore cleanup failure on partial load
      }
    }
    if (session && !detached) {
      try {
        await runWithTimeout(session.detach(), {
          timeoutMs: FRIDA_CLEANUP_TIMEOUT_MS,
          label: SERVER_MESSAGES.sessionDetachLabel,
        });
      } catch {
        // Ignore detach failure
      }
    }
    try {
      await runWithTimeout(frida.kill(pid), {
        timeoutMs: FRIDA_CLEANUP_TIMEOUT_MS,
        label: SERVER_MESSAGES.processKillLabel,
      });
    } catch {
      // Fallback to Win32 TerminateProcess below
    }
    if (isProcessAlive(pid)) {
      terminateProcessByPid(pid);
    }
    throw err;
  }

  const activeSession: Session = session;
  let cleanedUp: boolean = false;

  return {
    pid,
    session: activeSession,
    scripts,
    isDetached(): boolean {
      return detached;
    },
    detachReason(): string | null {
      return reasonStr;
    },
    async cleanup(): Promise<void> {
      if (cleanedUp) return;
      cleanedUp = true;
      for (const s of scripts) {
        try {
          if (!detached) {
            await runWithTimeout(s.unload(), {
              timeoutMs: FRIDA_CLEANUP_TIMEOUT_MS,
              label: SERVER_MESSAGES.scriptUnloadLabel,
            });
          }
        } catch {
          // Ignore unload error during teardown
        }
      }
      scripts.length = 0;
      try {
        if (!detached) {
          await runWithTimeout(activeSession.detach(), {
            timeoutMs: FRIDA_CLEANUP_TIMEOUT_MS,
            label: SERVER_MESSAGES.sessionDetachLabel,
          });
        }
      } catch {
        // Ignore detach error during teardown
      }
    },
  };
}
