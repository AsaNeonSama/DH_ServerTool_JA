import { resolve } from "path";
import frida, { MessageType, Stdio } from "frida";
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
} from "./constants";
import { println } from "./terminal";
import type {
  ActiveInjectionSession,
  PatchScript,
  SpawnAndInjectOptions,
} from "./types";
import {
  bindProcessToLauncherJob,
  isProcessAlive,
  terminateProcessByPid,
} from "./win32";

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

function runWithTimeout<T>(
  promise: Promise<T>,
  options: TimedOpOptions
): Promise<T> {
  const { timeoutMs, label, signal }: TimedOpOptions = options;
  if (signal?.aborted) {
    // Suppress potential late rejection from the abandoned operation
    promise.catch((): void => undefined);
    return Promise.reject(createAbortError(`${label} が中断されました`));
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
        rejectPromise(createAbortError(`${label} が中断されました`));
      };

      const timer: ReturnType<typeof setTimeout> = setTimeout((): void => {
        if (settled) return;
        settled = true;
        cleanup();
        rejectPromise(new Error(`${label} がタイムアウトしました (${timeoutMs}ms)`));
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
          rejectPromise(toError(err, `${label} に失敗しました`));
        }
      );
    }
  );
  return guarded;
}

function sleepDelay(ms: number, signal?: AbortSignal): Promise<void> {
  if (signal?.aborted) {
    return Promise.reject(createAbortError("注入処理が中断されました"));
  }
  const waiter: Promise<void> = new Promise(
    (resolveDelay: () => void, rejectDelay: (err: Error) => void): void => {
      const onAbort = (): void => {
        clearTimeout(timer);
        rejectDelay(createAbortError("注入処理が中断されました"));
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
      throw createAbortError("Fridaアタッチが中断されました");
    }
    try {
      return await runWithTimeout(frida.attach(pid), {
        timeoutMs: FRIDA_RPC_TIMEOUT_MS,
        label: `Fridaアタッチ (PID: ${pid})`,
        signal,
      });
    } catch (err: unknown) {
      lastError = err;
      if (signal?.aborted || !isProcessAlive(pid) || attempt >= FRIDA_ATTACH_MAX_RETRIES) {
        throw err;
      }
      await sleepDelay(FRIDA_ATTACH_RETRY_DELAY_MS * attempt, signal);
    }
  }
  throw toError(lastError, "Frida attach failed");
}

/**
 * Spawns DreadHungerServer-Win64-Shipping.exe directly in a suspended state
 * via Frida, injects all selected MOD scripts at 0s (before UE4 PreInit and
 * UObject::Serialize run), and then resumes the process.
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
    throw createAbortError("サーバー起動が中断されました");
  }

  const resolvedExe: string = resolve(shippingExecutable);
  const argv: string[] = [
    resolvedExe,
    UE_PROJECT_NAME,
    ueMapArgument,
    "-log",
    `-port=${port}`,
  ];

  println(
    `${C.LIGHT_CYAN} * ${shippingExecutable} を一時停止状態で直接起動中 (ポート: ${port})...`
  );

  const pid: number = await runWithTimeout(
    frida.spawn(argv, {
      cwd: process.cwd(),
      stdio: Stdio.Inherit,
    }),
    {
      timeoutMs: FRIDA_RPC_TIMEOUT_MS,
      label: "サーバープロセス生成",
      signal,
    }
  );

  bindProcessToLauncherJob(pid);

  println(
    `${C.LIGHT_GREEN} * プロセス生成完了 (PID: ${C.LIGHT_WHITE}${pid}${C.LIGHT_GREEN}) — パッチ注入を開始します`
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
          label: "コンソール初期化スクリプト生成",
          signal,
        }
      );
      await runWithTimeout(consoleBootstrap.load(), {
        timeoutMs: FRIDA_RPC_TIMEOUT_MS,
        label: "コンソール初期化スクリプト実行",
        signal,
      });
    } finally {
      if (consoleBootstrap && !detached) {
        try {
          await runWithTimeout(consoleBootstrap.unload(), {
            timeoutMs: FRIDA_CLEANUP_TIMEOUT_MS,
            label: "コンソール初期化スクリプト解放",
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
        throw createAbortError("パッチ注入が中断されました");
      }

      if (detached) {
        throw new Error(
          `プロセスから切断されたため注入を中断しました (${reasonStr ?? "detached"})`
        );
      }

      println(
        `${C.CYAN}   [${i + 1}/${patches.length}] 注入中: ${C.LIGHT_WHITE}${patch.label} ${C.LIGHT_BLACK}(${patch.fileName})`
      );

      const cleanSource: string =
        WINDOWS_NATIVE_FUNCTION_PRELUDE + patch.source.replace(/^\uFEFF/, "");
      const script: Script = await runWithTimeout(
        session.createScript(cleanSource, {
          name: patch.id,
        }),
        {
          timeoutMs: FRIDA_RPC_TIMEOUT_MS,
          label: `スクリプト生成 (${patch.fileName})`,
          signal,
        }
      );

      const loadState: { loading: boolean; error: string | null } = {
        loading: true,
        error: null,
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
          println(`${C.RED}   [PATCH ERROR:${patch.fileName}] ${desc}${stack}`);
        } else if (message.type === MessageType.Send) {
          const text: string = safeStringifyPayload(message.payload);
          println(`${C.LIGHT_CYAN}   [PATCH:${patch.fileName}] ${text}`);
        }
      });

      await runWithTimeout(script.load(), {
        timeoutMs: FRIDA_RPC_TIMEOUT_MS,
        label: `スクリプト注入 (${patch.fileName})`,
        signal,
      });
      loadState.loading = false;
      scripts.push(script);

      if (loadState.error !== null) {
        throw new Error(
          `パッチ初期化エラー (${patch.fileName}): ${loadState.error}`
        );
      }

      println(
        `${C.LIGHT_GREEN}   [${i + 1}/${patches.length}] 注入完了: ${patch.label}`
      );
    }

    if (signal?.aborted) {
      throw createAbortError("サーバー再開が中断されました");
    }

    await runWithTimeout(frida.resume(pid), {
      timeoutMs: FRIDA_RPC_TIMEOUT_MS,
      label: `サーバー再開 (PID: ${pid})`,
      signal,
    });
    println(
      `${C.LIGHT_GREEN} * 全 ${patches.length} 件のMODパッチの注入完了 -> サーバー実行を再開しました!`
    );
  } catch (err: unknown) {
    for (const s of scripts) {
      try {
        if (!detached) {
          await runWithTimeout(s.unload(), {
            timeoutMs: FRIDA_CLEANUP_TIMEOUT_MS,
            label: "スクリプト解放",
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
          label: "セッション切断",
        });
      } catch {
        // Ignore detach failure
      }
    }
    try {
      await runWithTimeout(frida.kill(pid), {
        timeoutMs: FRIDA_CLEANUP_TIMEOUT_MS,
        label: "プロセス終了",
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
              label: "スクリプト解放",
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
            label: "セッション切断",
          });
        }
      } catch {
        // Ignore detach error during teardown
      }
    },
  };
}
