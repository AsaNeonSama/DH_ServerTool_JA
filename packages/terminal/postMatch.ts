import { stdin as input, stdout as output } from "process";
import { C } from "../constants";
import { TERMINAL_MESSAGES } from "../messages";
import type { PostMatchAction } from "../types";
import {
  isTabbedModeActive,
  setServerKeyHandler,
  setServerStatusLine,
} from "./tabs";

function buildCountdownText(remaining: number): string {
  return (
    `${C.LIGHT_YELLOW} * ${TERMINAL_MESSAGES.postMatchCountdownPrefix} ` +
    `${C.LIGHT_GREEN}${remaining}${C.LIGHT_YELLOW} ${TERMINAL_MESSAGES.postMatchCountdownMid}` +
    `${C.LIGHT_CYAN}R${C.LIGHT_YELLOW}${TERMINAL_MESSAGES.postMatchCountdownReconfigure} ` +
    `${C.LIGHT_CYAN}Enter${C.LIGHT_YELLOW}${TERMINAL_MESSAGES.postMatchCountdownRestart}`
  );
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
        setServerStatusLine(buildCountdownText(remaining));
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
        output.write(`\r\x1b[K${buildCountdownText(remaining)}${C.RESET}`);
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
