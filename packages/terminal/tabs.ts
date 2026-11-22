import { stdin as input, stdout as output } from "process";
import { C, MAX_SERVER_TAB_LINES, MAX_WEB_LOG_LINES } from "../constants";
import { formatLocalTime } from "../core";
import { TERMINAL_MESSAGES } from "../messages";
import type {
  HitRegion,
  ServerKeyHandler,
  TerminalTabId,
  WebConsoleDisplayInfo,
} from "../types";
import {
  copyTextToClipboard,
  enableWindowsTabMouseInput,
  getWindowsInputMode,
  restoreWindowsInputMode,
} from "../win32";
import {
  countVisualRows,
  getDisplayWidth,
  selectVisibleLines,
} from "./display";
import type { VisibleLinesResult } from "./display";
import { parseEscapeSequence } from "./inputParser";
import type {
  EscapeActionCallbacks,
  EscapeParseResult,
  ParsedMouseEvent,
} from "./inputParser";
import {
  buildTabBar,
  buildWebLogBar,
  buildWebTabLayout,
  TAB_HEADER_ROWS,
} from "./tabRenderer";
import type {
  CopyFeedbackState,
  TabBarLayout,
  WebTabLayout,
} from "./tabRenderer";

interface ActivePromptState {
  readonly promptText: string;
  inputBuffer: string;
  readonly resolve: (value: string) => void;
  readonly reject: (err: Error) => void;
  readonly signal?: AbortSignal;
  readonly onAbort?: () => void;
}

const ESC_CODE: number = 0x1b;
const COPY_FEEDBACK_TTL_MS: number = 4000;
const ESC_FLUSH_DELAY_MS: number = 25;

let active: boolean = false;
let activeTab: TerminalTabId = "server";
let webConsoleInfo: WebConsoleDisplayInfo | null = null;
let savedInputMode: number | null = null;
let serverLines: string[] = [];
let scrollOffset: number = 0;
const webLogLines: string[] = [];
let webLogScrollOffset: number = 0;
let statusLine: string | null = null;
let activePrompt: ActivePromptState | null = null;
let serverKeyHandler: ServerKeyHandler | null = null;
let copyFeedback: CopyFeedbackState | null = null;
let copyFeedbackTimer: ReturnType<typeof setTimeout> | null = null;
let pendingEsc: string = "";
let pendingEscTimer: ReturnType<typeof setTimeout> | null = null;
let exitHookRegistered: boolean = false;

let currentTab1Hit: HitRegion | null = null;
let currentTab2Hit: HitRegion | null = null;
let currentPublicBtnHit: HitRegion | null = null;
let currentLocalBtnHit: HitRegion | null = null;

function safeStdoutWrite(data: string): void {
  try {
    output.write(data);
  } catch {
    // Ignore EPIPE / closed stdout errors
  }
}

export function isInteractiveTty(): boolean {
  return Boolean(input.isTTY && output.isTTY);
}

export function isTabbedModeActive(): boolean {
  return active;
}

function getTerminalSize(): { readonly cols: number; readonly rows: number } {
  const cols: number = Math.max(40, output.columns || 120);
  const rows: number = Math.max(10, output.rows || 30);
  return { cols, rows };
}

function isHit(region: HitRegion | null, col: number, row: number): boolean {
  if (!region) return false;
  return row === region.row && col >= region.startCol && col <= region.endCol;
}

function setCopyFeedback(text: string, isError: boolean): void {
  if (copyFeedbackTimer !== null) {
    clearTimeout(copyFeedbackTimer);
    copyFeedbackTimer = null;
  }
  copyFeedback = { text, isError };
  copyFeedbackTimer = setTimeout((): void => {
    copyFeedback = null;
    copyFeedbackTimer = null;
    if (active && activeTab === "web") {
      renderFrame();
    }
  }, COPY_FEEDBACK_TTL_MS);
  if (active && activeTab === "web") {
    renderFrame();
  }
}

function triggerCopyUrl(kind: "public" | "local"): void {
  if (!webConsoleInfo) return;
  const targetUrl: string =
    kind === "public" ? webConsoleInfo.publicUrl : webConsoleInfo.localUrl;
  const label: string =
    kind === "public"
      ? TERMINAL_MESSAGES.publicUrlShortLabel
      : TERMINAL_MESSAGES.localUrlShortLabel;
  const ok: boolean = copyTextToClipboard(targetUrl);
  if (ok) {
    setCopyFeedback(TERMINAL_MESSAGES.copySuccessFeedback(label), false);
    appendWebTabLine(
      ` ${C.LIGHT_BLACK}[${formatLocalTime()}] ${C.CYAN}[${TERMINAL_MESSAGES.copyLogCategory}] ${C.LIGHT_WHITE}${TERMINAL_MESSAGES.copyLogSuccess(label)}`
    );
  } else {
    setCopyFeedback(TERMINAL_MESSAGES.copyFailureFeedback, true);
    appendWebTabLine(
      ` ${C.LIGHT_BLACK}[${formatLocalTime()}] ${C.RED}[${TERMINAL_MESSAGES.copyLogCategory}] ${C.LIGHT_WHITE}${TERMINAL_MESSAGES.copyLogFailure(label)}`
    );
  }
}

function renderFrame(): void {
  if (!active) return;

  const {
    cols: termCols,
    rows: termRows,
  }: { readonly cols: number; readonly rows: number } = getTerminalSize();
  const bodyHeight: number = Math.max(3, termRows - TAB_HEADER_ROWS);

  let bodyLines: string[] = [];
  let cursorRow: number | null = null;
  let cursorCol: number | null = null;

  if (activeTab === "web") {
    const webLayout: WebTabLayout = buildWebTabLayout({
      termCols,
      webConsoleInfo,
      copyFeedback,
      hasPendingServerAction: activePrompt !== null || statusLine !== null,
    });
    currentPublicBtnHit = webLayout.publicBtnHit;
    currentLocalBtnHit = webLayout.localBtnHit;

    let usedRows: number = 0;
    for (const line of webLayout.lines) {
      const r: number = countVisualRows(line, termCols);
      if (usedRows + r > bodyHeight) {
        break;
      }
      bodyLines.push(`${line}${C.RESET}`);
      usedRows += r;
    }

    // Independent scrollable 512-line log pane below a blank line + dedicated bar
    const rowsAfterHeader: number = bodyHeight - usedRows;
    if (rowsAfterHeader >= 3) {
      const availableLogRows: number = rowsAfterHeader - 2; // 1 blank line + 1 divider bar
      const selected: VisibleLinesResult = selectVisibleLines({
        lines: webLogLines,
        currentOffset: webLogScrollOffset,
        availableRows: availableLogRows,
        termCols,
      });
      webLogScrollOffset = selected.clampedOffset;

      bodyLines.push("");
      bodyLines.push(
        buildWebLogBar({
          termCols,
          totalLines: webLogLines.length,
          scrollPos: webLogScrollOffset,
        })
      );

      if (selected.visibleLines.length === 0) {
        bodyLines.push(
          `${C.LIGHT_BLACK} ${TERMINAL_MESSAGES.webLogEmptyPlaceholder}${C.RESET}`
        );
      } else {
        for (const logLine of selected.visibleLines) {
          bodyLines.push(`${logLine}${C.RESET}`);
        }
      }
    }
  } else {
    currentPublicBtnHit = null;
    currentLocalBtnHit = null;

    const statusVisualRows: number = statusLine
      ? countVisualRows(statusLine, termCols)
      : 0;
    const promptLine: string | null = activePrompt
      ? `${activePrompt.promptText}${C.RESET}${activePrompt.inputBuffer}`
      : null;
    const promptPlainWidth: number = promptLine ? getDisplayWidth(promptLine) : 0;
    const promptVisualRows: number = promptLine
      ? Math.max(1, Math.floor(promptPlainWidth / termCols) + 1)
      : 0;

    const reservedRows: number = statusVisualRows + promptVisualRows;
    const availableLogRows: number = Math.max(1, bodyHeight - reservedRows);
    const selected: VisibleLinesResult = selectVisibleLines({
      lines: serverLines,
      currentOffset: scrollOffset,
      availableRows: availableLogRows,
      termCols,
    });
    scrollOffset = selected.clampedOffset;

    let currentVisualRow: number = TAB_HEADER_ROWS + 1;
    bodyLines = selected.visibleLines.map((line: string): string => {
      currentVisualRow += countVisualRows(line, termCols);
      return `${line}${C.RESET}`;
    });

    if (statusLine !== null && currentVisualRow + statusVisualRows - 1 <= termRows) {
      bodyLines.push(`${statusLine}${C.RESET}`);
      currentVisualRow += statusVisualRows;
    }

    if (promptLine !== null && currentVisualRow <= termRows) {
      const promptStartRow: number = currentVisualRow;
      bodyLines.push(promptLine);
      cursorRow = Math.min(
        termRows,
        promptStartRow + Math.floor(promptPlainWidth / termCols)
      );
      cursorCol = (promptPlainWidth % termCols) + 1;
    }
  }

  const tabBar: TabBarLayout = buildTabBar({
    termCols,
    activeTab,
    webConsoleInfo,
    serverScrollOffset: scrollOffset,
  });
  currentTab1Hit = tabBar.tab1Hit;
  currentTab2Hit = tabBar.tab2Hit;

  // Paint atomically with Synchronized Output Mode (Windows Terminal) + Home overwrite (0 flicker)
  let frame: string =
    `\x1b[?2026h\x1b[?25l\x1b[H` +
    `${tabBar.row1}\x1b[K\r\n` +
    `${tabBar.row2}\x1b[K\r\n` +
    `\x1b[J`;

  if (bodyLines.length > 0) {
    frame += bodyLines.join("\r\n");
  }

  if (cursorRow !== null && cursorCol !== null) {
    frame += `\x1b[${cursorRow};${cursorCol}H\x1b[?25h`;
  }
  frame += "\x1b[?2026l";

  safeStdoutWrite(frame);
}

function adjustActiveTabScroll(delta: number): void {
  if (activeTab === "web") {
    webLogScrollOffset = Math.max(0, webLogScrollOffset + delta);
  } else {
    scrollOffset = Math.max(0, scrollOffset + delta);
  }
  renderFrame();
}

function jumpActiveTabScroll(target: "top" | "bottom"): void {
  if (activeTab === "web") {
    webLogScrollOffset = target === "bottom" ? 0 : webLogLines.length;
  } else {
    scrollOffset = target === "bottom" ? 0 : serverLines.length;
  }
  renderFrame();
}

function handleMouseEvent(ev: ParsedMouseEvent): void {
  if (!ev.pressed) return;

  if (ev.button === 64) {
    adjustActiveTabScroll(3);
    return;
  }
  if (ev.button === 65) {
    adjustActiveTabScroll(-3);
    return;
  }

  if (ev.button !== 0) return;

  if (isHit(currentTab1Hit, ev.col, ev.row)) {
    switchTab("web");
    return;
  }
  if (isHit(currentTab2Hit, ev.col, ev.row)) {
    switchTab("server");
    return;
  }
  if (activeTab === "web") {
    if (isHit(currentPublicBtnHit, ev.col, ev.row)) {
      triggerCopyUrl("public");
      return;
    }
    if (isHit(currentLocalBtnHit, ev.col, ev.row)) {
      triggerCopyUrl("local");
    }
  }
}

const escapeCallbacks: EscapeActionCallbacks = {
  onSwitchTab: (tab: TerminalTabId): void => {
    switchTab(tab);
  },
  onScrollDelta: (delta: number): void => {
    adjustActiveTabScroll(delta);
  },
  onScrollJump: (target: "top" | "bottom"): void => {
    jumpActiveTabScroll(target);
  },
  onMouseEvent: (event: ParsedMouseEvent): void => {
    handleMouseEvent(event);
  },
};

function handleRegularChar(ch: string): void {
  const code: number = ch.codePointAt(0) ?? 0;

  // Ctrl+C always triggers graceful shutdown from any tab
  if (code === 0x03) {
    process.emit("SIGINT");
    return;
  }

  if (activeTab === "web") {
    const lower: string = ch.toLowerCase();
    if (lower === "p") {
      triggerCopyUrl("public");
    } else if (lower === "l") {
      triggerCopyUrl("local");
    }
    return;
  }

  // Tab 2: check custom key handler first (e.g. post-match countdown R / Enter)
  if (serverKeyHandler) {
    if (serverKeyHandler(ch)) {
      return;
    }
  }

  if (!activePrompt) return;

  if (ch === "\r" || ch === "\n") {
    const current: ActivePromptState = activePrompt;
    activePrompt = null;
    if (current.signal && current.onAbort) {
      current.signal.removeEventListener("abort", current.onAbort);
    }
    const committedAnswer: string = current.inputBuffer;
    scrollOffset = 0;
    appendServerTabLine(`${current.promptText}${C.RESET}${committedAnswer}`);
    current.resolve(committedAnswer.trim());
    return;
  }

  if (code === 0x7f || code === 0x08) {
    if (activePrompt.inputBuffer.length > 0) {
      const chars: string[] = Array.from(activePrompt.inputBuffer);
      chars.pop();
      activePrompt.inputBuffer = chars.join("");
      scrollOffset = 0;
      renderFrame();
    }
    return;
  }

  if (code === 0x15) {
    if (activePrompt.inputBuffer.length > 0) {
      activePrompt.inputBuffer = "";
      scrollOffset = 0;
      renderFrame();
    }
    return;
  }

  if (code >= 0x20) {
    activePrompt.inputBuffer += ch;
    scrollOffset = 0;
    renderFrame();
  }
}

function processInputBuffer(data: string): void {
  if (pendingEscTimer !== null) {
    clearTimeout(pendingEscTimer);
    pendingEscTimer = null;
  }

  const combined: string = pendingEsc + data;
  pendingEsc = "";

  let i: number = 0;
  while (i < combined.length) {
    if (combined.charCodeAt(i) === ESC_CODE) {
      const parsed: EscapeParseResult = parseEscapeSequence(
        combined,
        i,
        escapeCallbacks
      );
      if (parsed.incomplete) {
        pendingEsc = combined.slice(i);
        pendingEscTimer = setTimeout((): void => {
          pendingEsc = "";
          pendingEscTimer = null;
        }, ESC_FLUSH_DELAY_MS);
        break;
      }
      i += Math.max(1, parsed.consumedLength);
      continue;
    }

    const cp: number = combined.codePointAt(i) ?? 0;
    const ch: string = String.fromCodePoint(cp);
    i += ch.length;
    handleRegularChar(ch);
    if (!active) {
      break;
    }
  }
}

const onStdinData = (chunk: Buffer): void => {
  if (!active) return;
  processInputBuffer(chunk.toString("utf8"));
};

const onStdoutResize = (): void => {
  if (!active) return;
  renderFrame();
};

function restoreTerminalEmergency(): void {
  if (!active) return;
  active = false;
  try {
    output.write(`\x1b[?1006l\x1b[?1000l\x1b[?25h\x1b[?1049l${C.RESET}`);
  } catch {
    // Ignore write failure on exit
  }
  try {
    input.setRawMode(false);
  } catch {
    // Ignore TTY restore error
  }
  restoreWindowsInputMode(savedInputMode);
  savedInputMode = null;
}

export function switchTab(tab: TerminalTabId): void {
  if (!active || activeTab === tab) return;
  activeTab = tab;
  renderFrame();
}

export function enterTabbedMode(
  info: WebConsoleDisplayInfo,
  initialTab: TerminalTabId = "server"
): void {
  if (!isInteractiveTty() || active) return;

  webConsoleInfo = info;
  activeTab = initialTab;
  serverLines = [];
  scrollOffset = 0;
  webLogScrollOffset = 0;
  statusLine = null;
  activePrompt = null;
  serverKeyHandler = null;
  copyFeedback = null;
  pendingEsc = "";

  savedInputMode = getWindowsInputMode();
  try {
    input.setRawMode(true);
  } catch {
    // Fallback if raw mode unavailable
  }
  enableWindowsTabMouseInput();

  active = true;
  if (!exitHookRegistered) {
    exitHookRegistered = true;
    process.on("exit", restoreTerminalEmergency);
  }

  input.resume();
  input.on("data", onStdinData);
  output.on("resize", onStdoutResize);

  // Enter Alternate Screen Buffer + wipe scrollback + enable SGR mouse tracking
  safeStdoutWrite("\x1b[?1049h\x1b[3J\x1b[2J\x1b[H\x1b[?1000h\x1b[?1006h");
  renderFrame();
}

export function exitTabbedMode(): void {
  if (!active) return;

  if (copyFeedbackTimer !== null) {
    clearTimeout(copyFeedbackTimer);
    copyFeedbackTimer = null;
  }
  if (pendingEscTimer !== null) {
    clearTimeout(pendingEscTimer);
    pendingEscTimer = null;
  }
  pendingEsc = "";
  serverKeyHandler = null;
  statusLine = null;

  if (activePrompt) {
    const pending: ActivePromptState = activePrompt;
    activePrompt = null;
    if (pending.signal && pending.onAbort) {
      pending.signal.removeEventListener("abort", pending.onAbort);
    }
    const err: Error = new Error("Prompt aborted");
    err.name = "AbortError";
    pending.reject(err);
  }

  input.off("data", onStdinData);
  output.off("resize", onStdoutResize);
  restoreTerminalEmergency();
  input.pause();
}

export function appendWebTabLine(text: string): void {
  const splitLines: string[] = text.split(/\r?\n/);
  const addedCount: number = splitLines.length;
  for (const line of splitLines) {
    webLogLines.push(line);
  }
  let trimmedCount: number = 0;
  if (webLogLines.length > MAX_WEB_LOG_LINES) {
    trimmedCount = webLogLines.length - MAX_WEB_LOG_LINES;
    webLogLines.splice(0, trimmedCount);
  }
  if (webLogScrollOffset > 0) {
    webLogScrollOffset = Math.max(
      0,
      webLogScrollOffset + addedCount - trimmedCount
    );
  }
  if (active && activeTab === "web") {
    renderFrame();
  }
}

export function appendServerTabLine(text: string): void {
  const splitLines: string[] = text.split(/\r?\n/);
  for (const line of splitLines) {
    serverLines.push(line);
  }
  if (serverLines.length > MAX_SERVER_TAB_LINES) {
    serverLines.splice(0, serverLines.length - MAX_SERVER_TAB_LINES);
  }
  if (active && activeTab === "server") {
    renderFrame();
  }
}

export function resetServerTabRound(): void {
  serverLines = [];
  scrollOffset = 0;
  statusLine = null;
  if (active) {
    safeStdoutWrite("\x1b[3J");
    if (activeTab === "server") {
      renderFrame();
    }
  }
}

export function setServerStatusLine(line: string | null): void {
  statusLine = line;
  if (active && activeTab === "server") {
    renderFrame();
  }
}

export function setServerKeyHandler(handler: ServerKeyHandler | null): void {
  serverKeyHandler = handler;
}

export function askInTabbedMode(
  promptText: string,
  signal?: AbortSignal
): Promise<string> {
  if (signal?.aborted) {
    const err: Error = new Error("Prompt aborted");
    err.name = "AbortError";
    return Promise.reject(err);
  }

  const promptPromise: Promise<string> = new Promise(
    (resolve: (value: string) => void, reject: (err: Error) => void): void => {
      if (activePrompt) {
        const prev: ActivePromptState = activePrompt;
        activePrompt = null;
        if (prev.signal && prev.onAbort) {
          prev.signal.removeEventListener("abort", prev.onAbort);
        }
        const supersededErr: Error = new Error("Prompt aborted");
        supersededErr.name = "AbortError";
        prev.reject(supersededErr);
      }

      const onAbort = (): void => {
        if (activePrompt?.onAbort === onAbort) {
          activePrompt = null;
          if (active && activeTab === "server") {
            renderFrame();
          }
        }
        const err: Error = new Error("Prompt aborted");
        err.name = "AbortError";
        reject(err);
      };

      activePrompt = {
        promptText,
        inputBuffer: "",
        resolve,
        reject,
        signal,
        onAbort,
      };

      if (signal) {
        signal.addEventListener("abort", onAbort, { once: true });
      }

      scrollOffset = 0;
      if (active) {
        renderFrame();
      }
    }
  );

  return promptPromise;
}
