import { stdin as input, stdout as output } from "process";
import * as readline from "readline/promises";
import { C } from "./constants";
import type { PostMatchAction } from "./types";
import { initWindowsConsole } from "./win32";

export function setupTerminal(title: string): void {
  initWindowsConsole(title);
  if (process.stdout.isTTY) {
    process.stdout.write(`\x1b[2J\x1b[H\x1b]0;${title}\x07`);
  }
}

export function println(text: string = ""): void {
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
  private readonly rl: readline.Interface;

  constructor() {
    this.rl = readline.createInterface({ input, output });
  }

  async ask(promptText: string, signal?: AbortSignal): Promise<string> {
    const answer: string = signal
      ? await this.rl.question(`${promptText}${C.RESET}`, { signal })
      : await this.rl.question(`${promptText}${C.RESET}`);
    return answer.trim();
  }

  close(): void {
    this.rl.close();
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

export async function waitForPostMatchDecision(
  seconds: number,
  signal: AbortSignal
): Promise<PostMatchAction> {
  if (signal.aborted) {
    return "abort";
  }

  const totalSeconds: number = Math.max(1, Math.floor(seconds));

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
  let width: number = 0;
  for (const ch of text) {
    const code: number = ch.codePointAt(0) ?? 0;
    const isWide: boolean =
      (code >= 0x1100 && code <= 0x115f) ||
      (code >= 0x2e80 && code <= 0xa4cf && code !== 0x303f) ||
      (code >= 0xac00 && code <= 0xd7a3) ||
      (code >= 0xf900 && code <= 0xfaff) ||
      (code >= 0xfe10 && code <= 0xfe19) ||
      (code >= 0xfe30 && code <= 0xfe6f) ||
      (code >= 0xff00 && code <= 0xff60) ||
      (code >= 0xffe0 && code <= 0xffe6);
    width += isWide ? 2 : 1;
  }
  const pad: number = Math.max(0, targetWidth - width);
  return `${text}${" ".repeat(pad)}`;
}
