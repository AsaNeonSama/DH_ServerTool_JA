import { stdin as input, stdout as output } from "process";
import * as readline from "readline/promises";
import { C } from "./constants";
import {
  appendServerTabLine,
  askInTabbedMode,
  enterTabbedMode,
  exitTabbedMode,
  getDisplayWidth,
  isInteractiveTty,
  isTabbedModeActive,
  resetServerTabRound,
  setServerKeyHandler,
  setServerStatusLine,
} from "./terminalTabs";
import type { PostMatchAction } from "./types";
import { initWindowsConsole } from "./win32";

export {
  enterTabbedMode,
  exitTabbedMode,
  getDisplayWidth,
  isInteractiveTty,
  isTabbedModeActive,
  resetServerTabRound,
};

export function setupTerminal(title: string): void {
  initWindowsConsole(title);
  if (process.stdout.isTTY) {
    process.stdout.write(`\x1b[2J\x1b[H\x1b]0;${title}\x07`);
  }
}

export function println(text: string = ""): void {
  if (isTabbedModeActive()) {
    appendServerTabLine(text);
    return;
  }
  process.stdout.write(`${text}${C.RESET}\n`);
}

export function formatSeconds(ms: number): string {
  const seconds: number = ms / 1000;
  const formatted: string = Number.isInteger(seconds)
    ? String(seconds)
    : seconds.toFixed(2).replace(/\.?0+$/, "");
  return `${formatted}s`;
}

export class Prompter {
  private readonly rl: readline.Interface | null;

  constructor() {
    this.rl = isTabbedModeActive()
      ? null
      : readline.createInterface({ input, output });
  }

  async ask(promptText: string, signal?: AbortSignal): Promise<string> {
    if (isTabbedModeActive()) {
      return await askInTabbedMode(promptText, signal);
    }
    if (!this.rl) {
      throw new Error("Prompter is closed");
    }
    const answer: string = signal
      ? await this.rl.question(`${promptText}${C.RESET}`, { signal })
      : await this.rl.question(`${promptText}${C.RESET}`);
    return answer.trim();
  }

  close(): void {
    this.rl?.close();
  }
}

export async function waitForExitKey(): Promise<void> {
  if (!process.stdin.isTTY) return;
  const prompter: Prompter = new Prompter();
  try {
    await prompter.ask(`\n${C.LIGHT_YELLOW} * Enterキーを押して終了してください... `);
  } catch {
    // Ignore EOF when stdin closes
  } finally {
    prompter.close();
  }
}

function waitForPostMatchDecisionTabbed(
  totalSeconds: number,
  signal: AbortSignal
): Promise<PostMatchAction> {
  const decision: Promise<PostMatchAction> = new Promise(
    (resolve: (value: PostMatchAction) => void): void => {
      let remaining: number = totalSeconds;
      let done: boolean = false;

      const render = (): void => {
        setServerStatusLine(
          `${C.LIGHT_YELLOW} * 自動再起動まで残り ${C.LIGHT_GREEN}${remaining}${C.LIGHT_YELLOW} 秒... (${C.LIGHT_CYAN}R${C.LIGHT_YELLOW}: マップ・MOD再設定 / ${C.LIGHT_CYAN}Enter${C.LIGHT_YELLOW}: 今すぐ再起動)`
        );
      };

      const finish = (action: PostMatchAction): void => {
        if (done) return;
        done = true;
        clearInterval(timer);
        signal.removeEventListener("abort", onAbort);
        setServerKeyHandler(null);
        setServerStatusLine(null);
        resolve(action);
      };

      const onAbort = (): void => {
        finish("abort");
      };

      setServerKeyHandler((key: string): boolean => {
        if (key.toLowerCase() === "r") {
          finish("reconfigure");
          return true;
        }
        if (key === "\r" || key === "\n") {
          finish("restart");
          return true;
        }
        return false;
      });

      signal.addEventListener("abort", onAbort, { once: true });
      render();

      const timer: ReturnType<typeof setInterval> = setInterval((): void => {
        remaining -= 1;
        if (remaining <= 0) {
          finish("restart");
          return;
        }
        render();
      }, 1000);
    }
  );

  return decision;
}

export async function waitForPostMatchDecision(
  seconds: number,
  signal: AbortSignal
): Promise<PostMatchAction> {
  if (signal.aborted) {
    return "abort";
  }

  const totalSeconds: number = Math.max(1, Math.floor(seconds));

  if (isTabbedModeActive()) {
    return await waitForPostMatchDecisionTabbed(totalSeconds, signal);
  }

  if (!input.isTTY) {
    const nonTtyWait: Promise<PostMatchAction> = new Promise(
      (resolve: (value: PostMatchAction) => void): void => {
        const onAbort = (): void => {
          clearTimeout(timer);
          resolve("abort");
        };
        const timer: ReturnType<typeof setTimeout> = setTimeout((): void => {
          signal.removeEventListener("abort", onAbort);
          resolve("restart");
        }, totalSeconds * 1000);
        signal.addEventListener("abort", onAbort, { once: true });
      }
    );
    return nonTtyWait;
  }

  const decision: Promise<PostMatchAction> = new Promise(
    (resolve: (value: PostMatchAction) => void): void => {
      let remaining: number = totalSeconds;
      let done: boolean = false;

      const render = (): void => {
        output.write(
          `\r\x1b[K${C.LIGHT_YELLOW} * 自動再起動まで残り ${C.LIGHT_GREEN}${remaining}${C.LIGHT_YELLOW} 秒... (${C.LIGHT_CYAN}R${C.LIGHT_YELLOW}: マップ・MOD再設定 / ${C.LIGHT_CYAN}Enter${C.LIGHT_YELLOW}: 今すぐ再起動)${C.RESET}`
        );
      };

      const finish = (action: PostMatchAction): void => {
        if (done) return;
        done = true;
        clearInterval(timer);
        signal.removeEventListener("abort", onAbort);
        input.off("data", onData);
        try {
          input.setRawMode(false);
        } catch {
          // Ignore if TTY mode already restored
        }
        input.pause();
        output.write("\n");
        resolve(action);
      };

      const onAbort = (): void => {
        finish("abort");
      };

      const onData = (chunk: Buffer): void => {
        const text: string = chunk.toString("utf8");
        if (text.includes("\x03")) {
          finish("abort");
          process.emit("SIGINT");
          return;
        }
        if (text.toLowerCase().includes("r")) {
          finish("reconfigure");
          return;
        }
        if (text.includes("\r") || text.includes("\n")) {
          finish("restart");
        }
      };

      signal.addEventListener("abort", onAbort, { once: true });
      try {
        input.setRawMode(true);
      } catch {
        // Fallback if raw mode unavailable
      }
      input.resume();
      input.on("data", onData);
      render();

      const timer: ReturnType<typeof setInterval> = setInterval((): void => {
        remaining -= 1;
        if (remaining <= 0) {
          finish("restart");
          return;
        }
        render();
      }, 1000);
    }
  );

  return decision;
}

export function padDisplayWidth(text: string, targetWidth: number): string {
  const width: number = getDisplayWidth(text);
  const pad: number = Math.max(0, targetWidth - width);
  return `${text}${" ".repeat(pad)}`;
}
