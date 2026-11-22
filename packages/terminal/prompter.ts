import { stdin as input, stdout as output } from "process";
import * as readline from "readline/promises";
import { C } from "../constants";
import { TERMINAL_MESSAGES } from "../messages";
import { initWindowsConsole } from "../win32";
import {
  appendServerTabLine,
  askInTabbedMode,
  isTabbedModeActive,
} from "./tabs";

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
    await prompter.ask(
      `\n${C.LIGHT_YELLOW} * ${TERMINAL_MESSAGES.pressEnterToExit} `
    );
  } catch {
    // Ignore EOF when stdin closes
  } finally {
    prompter.close();
  }
}
