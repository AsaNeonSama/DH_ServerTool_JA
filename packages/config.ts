import { existsSync } from "fs";
import { dirname, join } from "path";
import {
  C,
  CONFIG_RELATIVE_PATH,
  DEFAULT_CONFIG,
  MAX_VALID_PORT,
  MIN_CHECK_INTERVAL_MS,
  MIN_RESTART_WAIT_SECONDS,
  MIN_VALID_PORT,
} from "./constants";
import { println } from "./terminal";
import type { AppConfig, RawConfig } from "./types";

function isValidPort(n: unknown): n is number {
  return (
    typeof n === "number" &&
    Number.isInteger(n) &&
    n >= MIN_VALID_PORT &&
    n <= MAX_VALID_PORT
  );
}

function cloneDefaultConfig(): AppConfig {
  return {
    ...DEFAULT_CONFIG,
    serverPorts: [DEFAULT_CONFIG.serverPorts[0], DEFAULT_CONFIG.serverPorts[1]],
  };
}

function readNonEmptyString(value: unknown, fallback: string): string {
  if (typeof value === "string" && value.trim().length > 0) {
    return value.trim();
  }
  return fallback;
}

function readMinInt(value: unknown, minValue: number, fallback: number): number {
  if (typeof value === "number" && Number.isFinite(value) && value >= minValue) {
    return Math.floor(value);
  }
  return fallback;
}

/**
 * Ensures process.cwd() points to the WindowsServer directory containing
 * the server executable using relative paths only.
 */
export function ensureServerWorkingDir(serverExecutable: string): boolean {
  if (existsSync(serverExecutable)) {
    return true;
  }
  const exeDir: string = dirname(process.execPath);
  if (exeDir && existsSync(join(exeDir, serverExecutable))) {
    process.chdir(exeDir);
    return existsSync(serverExecutable);
  }
  return false;
}

export async function loadOrCreateConfig(): Promise<AppConfig> {
  if (!existsSync(CONFIG_RELATIVE_PATH)) {
    try {
      await Bun.write(
        CONFIG_RELATIVE_PATH,
        `${JSON.stringify(DEFAULT_CONFIG, null, 2)}\n`
      );
      println(
        `${C.LIGHT_CYAN} * デフォルト設定ファイル (${CONFIG_RELATIVE_PATH}) を作成しました`
      );
    } catch {
      // Non-fatal if directory is read-only
    }
    return cloneDefaultConfig();
  }

  try {
    const raw: unknown = await Bun.file(CONFIG_RELATIVE_PATH).json();
    const parsed: RawConfig =
      typeof raw === "object" && raw !== null ? raw : {};

    let serverPorts: readonly [number, number] = [
      DEFAULT_CONFIG.serverPorts[0],
      DEFAULT_CONFIG.serverPorts[1],
    ];
    if (
      Array.isArray(parsed.serverPorts) &&
      parsed.serverPorts.length >= 2 &&
      isValidPort(parsed.serverPorts[0]) &&
      isValidPort(parsed.serverPorts[1]) &&
      parsed.serverPorts[0] !== parsed.serverPorts[1]
    ) {
      serverPorts = [parsed.serverPorts[0], parsed.serverPorts[1]];
    }

    const consolePatchPort: number = isValidPort(parsed.consolePatchPort)
      ? parsed.consolePatchPort
      : DEFAULT_CONFIG.consolePatchPort;

    const webConsolePort: number = isValidPort(parsed.webConsolePort)
      ? parsed.webConsolePort
      : DEFAULT_CONFIG.webConsolePort;

    const checkIntervalMs: number = readMinInt(
      parsed.checkIntervalMs,
      MIN_CHECK_INTERVAL_MS,
      DEFAULT_CONFIG.checkIntervalMs
    );
    const restartWaitSeconds: number = readMinInt(
      parsed.restartWaitSeconds,
      MIN_RESTART_WAIT_SECONDS,
      DEFAULT_CONFIG.restartWaitSeconds
    );
    const injectDelayMs: number = readMinInt(
      parsed.injectDelayMs,
      0,
      DEFAULT_CONFIG.injectDelayMs
    );
    const scriptGapMs: number = readMinInt(
      parsed.scriptGapMs,
      0,
      DEFAULT_CONFIG.scriptGapMs
    );
    const autoBroadcastDefault: boolean =
      typeof parsed.autoBroadcastDefault === "boolean"
        ? parsed.autoBroadcastDefault
        : DEFAULT_CONFIG.autoBroadcastDefault;

    return {
      serverPorts,
      consolePatchPort,
      webConsolePort,
      checkIntervalMs,
      restartWaitSeconds,
      injectDelayMs,
      scriptGapMs,
      autoBroadcastDefault,
      serverExecutable: readNonEmptyString(
        parsed.serverExecutable,
        DEFAULT_CONFIG.serverExecutable
      ),
      shippingExecutable: readNonEmptyString(
        parsed.shippingExecutable,
        DEFAULT_CONFIG.shippingExecutable
      ),
      shippingProcessName: readNonEmptyString(
        parsed.shippingProcessName,
        DEFAULT_CONFIG.shippingProcessName
      ),
    };
  } catch (err: unknown) {
    const msg: string = err instanceof Error ? err.message : String(err);
    println(
      `${C.RED} * config.json の読み込みに失敗しました。デフォルト設定を使用します: ${msg}`
    );
    return cloneDefaultConfig();
  }
}
