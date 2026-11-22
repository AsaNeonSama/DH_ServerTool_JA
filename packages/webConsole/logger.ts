import { ACTOR_COMMENT_DICT, C, STATUS_CODE } from "../constants";
import { formatLocalTime, sanitizeLogText } from "../core";
import { WEB_CONSOLE_MESSAGES } from "../messages";
import { appendWebTabLine } from "../terminal";

export const MAX_LOG_TEXT_CHARS: number = 64;

export interface WebLogEntryOptions {
  readonly category: string;
  readonly categoryColor: string;
  readonly detail: string;
}

export function normalizeClientIp(rawIp: string | null | undefined): string {
  if (!rawIp) return "unknown";
  let ip: string = rawIp.trim();
  if (ip.startsWith("::ffff:")) {
    ip = ip.slice(7);
  }
  if (ip === "::1") {
    ip = "127.0.0.1";
  }
  const safe: string = sanitizeLogText(ip, 45);
  return safe.length > 0 ? safe : "unknown";
}

export function formatResultBadge(status: number, errDetail?: string): string {
  if (status === STATUS_CODE.success) {
    return `${C.LIGHT_GREEN}${WEB_CONSOLE_MESSAGES.resultBadges.sent}`;
  }
  if (status === STATUS_CODE.partialSuccess) {
    return `${C.LIGHT_YELLOW}${WEB_CONSOLE_MESSAGES.resultBadges.partialFailure}`;
  }
  const suffix: string =
    errDetail && errDetail.length > 0
      ? `: ${sanitizeLogText(errDetail, 36)}`
      : "";
  if (status === STATUS_CODE.serverError) {
    return `${C.RED}${WEB_CONSOLE_MESSAGES.resultBadges.sendFailed(suffix)}`;
  }
  return `${C.RED}${WEB_CONSOLE_MESSAGES.resultBadges.invalid(suffix)}`;
}

export function emitWebLog(options: WebLogEntryOptions): void {
  const { category, categoryColor, detail }: WebLogEntryOptions = options;
  appendWebTabLine(
    ` ${C.LIGHT_BLACK}[${formatLocalTime()}] ${categoryColor}[${category}] ${C.LIGHT_WHITE}${detail}`
  );
}

export function formatConsoleCommandPreview(command: string): string {
  const spaceIdx: number = command.indexOf(" ");
  const cmdName: string = spaceIdx === -1 ? command : command.slice(0, spaceIdx);
  const cmdArg: string =
    spaceIdx === -1 ? "" : command.slice(spaceIdx + 1).trim();
  if (cmdName === "summon" && cmdArg.length > 0) {
    const knownName: string | undefined = ACTOR_COMMENT_DICT[cmdArg];
    if (knownName) {
      return `summon ${knownName}`;
    }
  }
  return sanitizeLogText(command, MAX_LOG_TEXT_CHARS);
}
