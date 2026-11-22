import { appendFileSync, writeFileSync } from "fs";
import { PATCH_RUNTIME_LOG_RELATIVE_PATH } from "../constants";
import { formatLogDateTime } from "../core";
import { SERVER_MESSAGES } from "../messages";
import type { PatchScript } from "../types";

export type PatchLogLevel = "INFO" | "WARN" | "ERROR";

export interface ResetPatchLogOptions {
  readonly port: number;
  readonly patches: readonly PatchScript[];
}

export interface PatchLogEntry {
  readonly level: PatchLogLevel;
  readonly fileName: string;
  readonly message: string;
}

/**
 * Overwrites `./patch_runtime.log` in the launcher/server root directory
 * at the start of every match round so only the latest match is kept.
 */
export function resetPatchRuntimeLog(options: ResetPatchLogOptions): void {
  const { port, patches }: ResetPatchLogOptions = options;
  const patchNames: string = patches
    .map((patch: PatchScript): string => patch.fileName)
    .join(", ");
  const header: string = SERVER_MESSAGES.patchLogHeader(
    formatLogDateTime(),
    port,
    patchNames
  );
  try {
    writeFileSync(PATCH_RUNTIME_LOG_RELATIVE_PATH, header, { encoding: "utf8" });
  } catch {
    // Non-fatal if working directory is read-only or file is temporarily locked
  }
}

export function appendPatchRuntimeLog(entry: PatchLogEntry): void {
  const { level, fileName, message }: PatchLogEntry = entry;
  const line: string = `[${formatLogDateTime()}] [${level}] [${fileName}] ${message}\n`;
  try {
    appendFileSync(PATCH_RUNTIME_LOG_RELATIVE_PATH, line, { encoding: "utf8" });
  } catch {
    // Ignore transient file I/O errors inside Frida message callbacks
  }
}
