import { resolve } from "path";
import frida, { MessageType, Stdio } from "frida";
import type { Message, Script, Session, SessionDetachReason } from "frida";
import {
  C,
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
import { terminateProcessByPid } from "./win32";

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
    scriptGapMs,
    onDetached,
  }: SpawnAndInjectOptions = options;

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

  const pid: number = await frida.spawn(argv, {
    cwd: process.cwd(),
    stdio: Stdio.Inherit,
  });

  println(
    `${C.LIGHT_GREEN} * プロセス生成完了 (PID: ${C.LIGHT_WHITE}${pid}${C.LIGHT_GREEN}) — パッチ注入を開始します`
  );

  let session: Session | null = null;
  let detached: boolean = false;
  let reasonStr: string | null = null;
  const scripts: Script[] = [];

  try {
    session = await frida.attach(pid);

    session.detached.connect((reason: SessionDetachReason): void => {
      detached = true;
      reasonStr = String(reason);
      onDetached?.(reasonStr);
    });

    // Clear inherited STARTF_USESTDHANDLES so UE4's -log AllocConsole() writes to the new server console
    const consoleBootstrap: Script = await session.createScript(
      WINDOWS_CONSOLE_BOOTSTRAP_SCRIPT,
      { name: "runtime:windows_console_bootstrap" }
    );
    await consoleBootstrap.load();
    await consoleBootstrap.unload();

    for (let i: number = 0; i < patches.length; i++) {
      const patch: PatchScript | undefined = patches[i];
      if (!patch) continue;

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
      const script: Script = await session.createScript(cleanSource, {
        name: patch.id,
      });

      script.message.connect((message: Message): void => {
        if (message.type === MessageType.Error) {
          const desc: string =
            "description" in message && typeof message.description === "string"
              ? message.description
              : JSON.stringify(message);
          const stack: string =
            "stack" in message && typeof message.stack === "string"
              ? `\n${message.stack}`
              : "";
          println(`${C.RED}   [PATCH ERROR:${patch.fileName}] ${desc}${stack}`);
        } else if (message.type === MessageType.Send) {
          const text: string =
            typeof message.payload === "string"
              ? message.payload
              : JSON.stringify(message.payload);
          println(`${C.LIGHT_CYAN}   [PATCH:${patch.fileName}] ${text}`);
        }
      });

      await script.load();
      scripts.push(script);

      println(
        `${C.LIGHT_GREEN}   [${i + 1}/${patches.length}] 注入完了: ${patch.label}`
      );

      if (scriptGapMs > 0 && i < patches.length - 1) {
        await Bun.sleep(scriptGapMs);
      }
    }

    await frida.resume(pid);
    println(
      `${C.LIGHT_GREEN} * 全 ${patches.length} 件のMODパッチの注入完了 -> サーバー実行を再開しました!`
    );
  } catch (err: unknown) {
    for (const s of scripts) {
      try {
        if (!detached) {
          await s.unload();
        }
      } catch {
        // Ignore cleanup failure on partial load
      }
    }
    if (session && !detached) {
      try {
        await session.detach();
      } catch {
        // Ignore detach failure
      }
    }
    try {
      await frida.kill(pid);
    } catch {
      terminateProcessByPid(pid);
    }
    throw err;
  }

  const activeSession: Session = session;

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
      for (const s of scripts) {
        try {
          if (!detached) {
            await s.unload();
          }
        } catch {
          // Ignore unload error during teardown
        }
      }
      scripts.length = 0;
      try {
        if (!detached) {
          await activeSession.detach();
        }
      } catch {
        // Ignore detach error during teardown
      }
    },
  };
}
