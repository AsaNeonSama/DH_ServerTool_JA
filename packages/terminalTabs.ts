import { stdin as input, stdout as output } from "process";
import {
  C,
  MAX_SERVER_TAB_LINES,
  TAB_ACTIVE_STYLE,
  TAB_BUTTON_STYLE,
  TAB_INACTIVE_STYLE,
} from "./constants";
import type {
  HitRegion,
  ServerKeyHandler,
  TerminalTabId,
  WebConsoleDisplayInfo,
} from "./types";
import {
  copyTextToClipboard,
  enableWindowsTabMouseInput,
  getWindowsInputMode,
  restoreWindowsInputMode,
} from "./win32";

interface ActivePromptState {
  readonly promptText: string;
  inputBuffer: string;
  readonly resolve: (value: string) => void;
  readonly reject: (err: Error) => void;
  readonly signal?: AbortSignal;
  readonly onAbort?: () => void;
}

interface CopyFeedbackState {
  readonly text: string;
  readonly isError: boolean;
}

interface ParsedMouseEvent {
  readonly button: number;
  readonly col: number;
  readonly row: number;
  readonly pressed: boolean;
  readonly consumedLength: number;
}

interface EscapeParseResult {
  readonly consumedLength: number;
  readonly incomplete: boolean;
}

interface TabBarLayout {
  readonly row1: string;
  readonly row2: string;
  readonly tab1Hit: HitRegion;
  readonly tab2Hit: HitRegion;
}

interface WebTabLayout {
  readonly lines: string[];
  readonly publicBtnHit: HitRegion | null;
  readonly localBtnHit: HitRegion | null;
}

const ESC_CODE: number = 0x1b;
const TAB_HEADER_ROWS: number = 2;
const COPY_FEEDBACK_TTL_MS: number = 4000;
const ESC_FLUSH_DELAY_MS: number = 25;

let active: boolean = false;
let activeTab: TerminalTabId = "server";
let webConsoleInfo: WebConsoleDisplayInfo | null = null;
let savedInputMode: number | null = null;
let serverLines: string[] = [];
let scrollOffset: number = 0;
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

export function isInteractiveTty(): boolean {
  return Boolean(input.isTTY && output.isTTY);
}

export function isTabbedModeActive(): boolean {
  return active;
}

export function stripAnsi(text: string): string {
  let result: string = "";
  let i: number = 0;
  while (i < text.length) {
    if (text.charCodeAt(i) === ESC_CODE && text[i + 1] === "[") {
      i += 2;
      while (i < text.length) {
        const code: number = text.charCodeAt(i);
        i += 1;
        if (code >= 0x40 && code <= 0x7e) {
          break;
        }
      }
      continue;
    }
    result += text[i] ?? "";
    i += 1;
  }
  return result;
}

export function getDisplayWidth(text: string): number {
  const plain: string = stripAnsi(text);
  let width: number = 0;
  for (const ch of plain) {
    const code: number = ch.codePointAt(0) ?? 0;
    if (code < 0x20 || code === 0x7f) {
      continue;
    }
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
  return width;
}

function countVisualRows(line: string, termCols: number): number {
  const width: number = getDisplayWidth(line);
  if (width === 0) return 1;
  return Math.max(1, Math.ceil(width / Math.max(1, termCols)));
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
    kind === "public" ? "パブリックアドレス" : "ローカルアドレス";
  const ok: boolean = copyTextToClipboard(targetUrl);
  if (ok) {
    setCopyFeedback(` * ✓ ${label}をクリップボードにコピーしました!`, false);
  } else {
    setCopyFeedback(
      ` * クリップボードへのコピーに失敗しました (Shift+ドラッグで選択してください)`,
      true
    );
  }
}

function buildTabBar(termCols: number): TabBarLayout {
  const portSuffix: string = webConsoleInfo ? ` (${webConsoleInfo.port})` : "";
  const tab1Text: string = ` [1] Webコンソール${portSuffix} `;
  const tab2Text: string = " [2] サーバー・試合設定 ";
  const sepText: string = " │ ";
  const hintText: string = "  [←/→・クリック切替]";

  const tab1Width: number = getDisplayWidth(tab1Text);
  const sepWidth: number = getDisplayWidth(sepText);
  const tab2Width: number = getDisplayWidth(tab2Text);
  const hintWidth: number = getDisplayWidth(hintText);

  const tab1StartCol: number = 2;
  const tab1EndCol: number = tab1StartCol + tab1Width - 1;
  const tab2StartCol: number = tab1EndCol + sepWidth + 1;
  const tab2EndCol: number = tab2StartCol + tab2Width - 1;

  const tab1Style: string =
    activeTab === "web" ? TAB_ACTIVE_STYLE : TAB_INACTIVE_STYLE;
  const tab2Style: string =
    activeTab === "server" ? TAB_ACTIVE_STYLE : TAB_INACTIVE_STYLE;

  const baseWidth: number = 1 + tab1Width + sepWidth + tab2Width;
  const showHint: boolean = baseWidth + hintWidth <= termCols;
  const hintSegment: string = showHint
    ? `${C.LIGHT_BLACK}${hintText}${C.RESET}`
    : "";

  const row1: string =
    ` ${tab1Style}${tab1Text}${C.RESET}` +
    `${C.LIGHT_BLACK}${sepText}${C.RESET}` +
    `${tab2Style}${tab2Text}${C.RESET}` +
    hintSegment;

  let row2: string = `${C.LIGHT_BLACK}${"─".repeat(termCols)}${C.RESET}`;
  if (activeTab === "server" && scrollOffset > 0) {
    const scrollBadge: string = ` ▲ ${scrollOffset}行上を表示中 (ホイール/PgDnで最新へ) `;
    const badgeWidth: number = getDisplayWidth(scrollBadge);
    if (badgeWidth + 4 < termCols) {
      const leftDashes: number = Math.max(2, termCols - badgeWidth - 2);
      row2 =
        `${C.LIGHT_BLACK}${"─".repeat(leftDashes)}${C.LIGHT_YELLOW}${scrollBadge}` +
        `${C.LIGHT_BLACK}──${C.RESET}`;
    }
  }

  return {
    row1,
    row2,
    tab1Hit: { row: 1, startCol: tab1StartCol, endCol: tab1EndCol },
    tab2Hit: { row: 1, startCol: tab2StartCol, endCol: tab2EndCol },
  };
}

function buildWebTabLayout(termCols: number): WebTabLayout {
  if (!webConsoleInfo) {
    return { lines: [], publicBtnHit: null, localBtnHit: null };
  }

  const broadcastText: string = webConsoleInfo.autoBroadcast ? "オン" : "オフ";
  const prefixLines: string[] = [
    `${C.LIGHT_CYAN} -------------------- [Webコンソール設定] --------------------`,
    `${C.LIGHT_WHITE} * TCPポート: ${C.LIGHT_GREEN}${webConsoleInfo.port}  ${C.LIGHT_WHITE}/ 自動ブロードキャスト: ${C.LIGHT_GREEN}${broadcastText}`,
    "",
    `${C.LIGHT_CYAN} ==================== Webコンソール起動 ====================`,
    `${C.LIGHT_BLACK} * パブリックアドレス: ${webConsoleInfo.publicUrl}`,
    `${C.LIGHT_BLACK} * ローカルアドレス:   ${webConsoleInfo.localUrl}`,
    `${C.LIGHT_YELLOW} * URL末尾の #PSK は事前共有鍵(AES-256-GCM)です。${C.LIGHT_WHITE}全API通信が密文で保護されます。${C.LIGHT_MAGENTA}信用していない人に渡さないで下さい`,
    `${C.LIGHT_BLACK} * 内部連携ポート(console_win64.js): 127.0.0.1:${webConsoleInfo.consolePatchPort} / 自動ブロードキャスト: ${broadcastText}`,
    `${C.LIGHT_CYAN} ===========================================================`,
    "",
  ];

  let visualRowCursor: number = TAB_HEADER_ROWS + 1;
  for (const line of prefixLines) {
    visualRowCursor += countVisualRows(line, termCols);
  }

  const pubBtnLabel: string = " パブリックURLコピー (P) ";
  const locBtnLabel: string = " ローカルURLコピー (L) ";
  const leftPad: string = "  ";
  const gap: string = "   ";

  const pubStartCol: number = getDisplayWidth(leftPad) + 1;
  const pubEndCol: number = pubStartCol + getDisplayWidth(pubBtnLabel) - 1;
  const locStartCol: number = pubEndCol + getDisplayWidth(gap) + 1;
  const locEndCol: number = locStartCol + getDisplayWidth(locBtnLabel) - 1;

  const buttonLine: string =
    `${leftPad}${TAB_BUTTON_STYLE}${pubBtnLabel}${C.RESET}` +
    `${gap}${TAB_BUTTON_STYLE}${locBtnLabel}${C.RESET}`;

  const suffixLines: string[] = [buttonLine];
  if (copyFeedback) {
    const fbColor: string = copyFeedback.isError ? C.RED : C.LIGHT_GREEN;
    suffixLines.push(`${fbColor}${copyFeedback.text}`);
  }
  suffixLines.push(
    `${C.LIGHT_BLACK} * ボタンクリックまたは P / L キーでURLをコピー (手動選択は Shift+ドラッグ)`
  );
  if (activePrompt || statusLine) {
    suffixLines.push(
      `${C.LIGHT_YELLOW} * ※「サーバー・試合設定」タブで入力・操作待ちです (→キーまたはタブクリックで切替)`
    );
  } else {
    suffixLines.push(
      `${C.LIGHT_BLACK} * → キーまたは「サーバー・試合設定」タブをクリックしてサーバー画面へ切替`
    );
  }

  const buttonRowFits: boolean = locEndCol <= termCols;
  return {
    lines: [...prefixLines, ...suffixLines],
    publicBtnHit: buttonRowFits
      ? { row: visualRowCursor, startCol: pubStartCol, endCol: pubEndCol }
      : null,
    localBtnHit: buttonRowFits
      ? { row: visualRowCursor, startCol: locStartCol, endCol: locEndCol }
      : null,
  };
}

function computeMaxScrollOffset(availableRows: number, termCols: number): number {
  let totalRows: number = 0;
  for (const line of serverLines) {
    totalRows += countVisualRows(line, termCols);
  }
  if (totalRows <= availableRows) {
    return 0;
  }
  let usedFromTop: number = 0;
  let fitCountFromTop: number = 0;
  for (const line of serverLines) {
    const r: number = countVisualRows(line, termCols);
    if (usedFromTop + r > availableRows && fitCountFromTop > 0) {
      break;
    }
    usedFromTop += r;
    fitCountFromTop += 1;
  }
  return Math.max(0, serverLines.length - fitCountFromTop);
}

function selectVisibleServerLines(
  availableRows: number,
  termCols: number
): string[] {
  if (serverLines.length === 0) {
    scrollOffset = 0;
    return [];
  }

  const maxOffset: number = computeMaxScrollOffset(availableRows, termCols);
  if (scrollOffset > maxOffset) {
    scrollOffset = maxOffset;
  }

  const endIdx: number = Math.max(0, serverLines.length - scrollOffset);
  const selected: string[] = [];
  let usedRows: number = 0;

  for (let i: number = endIdx - 1; i >= 0; i--) {
    const line: string | undefined = serverLines[i];
    if (line === undefined) continue;
    const rowsNeeded: number = countVisualRows(line, termCols);
    if (usedRows + rowsNeeded > availableRows && selected.length > 0) {
      break;
    }
    selected.push(line);
    usedRows += rowsNeeded;
    if (usedRows >= availableRows) {
      break;
    }
  }

  selected.reverse();
  return selected;
}

function renderFrame(): void {
  if (!active) return;

  const { cols: termCols, rows: termRows }: { readonly cols: number; readonly rows: number } =
    getTerminalSize();
  const bodyHeight: number = Math.max(3, termRows - TAB_HEADER_ROWS);

  const tabBar: TabBarLayout = buildTabBar(termCols);
  currentTab1Hit = tabBar.tab1Hit;
  currentTab2Hit = tabBar.tab2Hit;

  let bodyLines: string[] = [];
  let cursorRow: number | null = null;
  let cursorCol: number | null = null;

  if (activeTab === "web") {
    const webLayout: WebTabLayout = buildWebTabLayout(termCols);
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
    const visibleLogs: string[] = selectVisibleServerLines(availableLogRows, termCols);

    let currentVisualRow: number = TAB_HEADER_ROWS + 1;
    bodyLines = visibleLogs.map((line: string): string => {
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

  output.write(frame);
}

function parseSgrMouse(buf: string, offset: number): ParsedMouseEvent | null {
  if (
    buf.charCodeAt(offset) !== ESC_CODE ||
    buf[offset + 1] !== "[" ||
    buf[offset + 2] !== "<"
  ) {
    return null;
  }
  let idx: number = offset + 3;
  let body: string = "";
  while (idx < buf.length) {
    const ch: string = buf[idx] ?? "";
    idx += 1;
    if (ch === "M" || ch === "m") {
      const parts: string[] = body.split(";");
      if (parts.length !== 3) return null;
      const button: number = Number(parts[0]);
      const col: number = Number(parts[1]);
      const row: number = Number(parts[2]);
      if (
        !Number.isInteger(button) ||
        !Number.isInteger(col) ||
        !Number.isInteger(row)
      ) {
        return null;
      }
      return {
        button,
        col,
        row,
        pressed: ch === "M",
        consumedLength: idx - offset,
      };
    }
    if ((ch >= "0" && ch <= "9") || ch === ";") {
      body += ch;
      if (body.length > 24) return null;
    } else {
      return null;
    }
  }
  return null;
}

function handleMouseEvent(ev: ParsedMouseEvent): void {
  if (!ev.pressed) return;

  if (ev.button === 64 && activeTab === "server") {
    scrollOffset += 3;
    renderFrame();
    return;
  }
  if (ev.button === 65 && activeTab === "server") {
    scrollOffset = Math.max(0, scrollOffset - 3);
    renderFrame();
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

function parseEscapeSequence(buf: string, offset: number): EscapeParseResult {
  const remaining: number = buf.length - offset;
  if (remaining === 1) {
    return { consumedLength: 0, incomplete: true };
  }

  const next: string = buf[offset + 1] ?? "";

  // SS3 sequences: ESC O <final>
  if (next === "O") {
    if (remaining < 3) {
      return { consumedLength: 0, incomplete: true };
    }
    const finalCh: string = buf[offset + 2] ?? "";
    if (finalCh === "D") {
      switchTab("web");
    } else if (finalCh === "C") {
      switchTab("server");
    } else if (finalCh === "A" && activeTab === "server") {
      scrollOffset += 1;
      renderFrame();
    } else if (finalCh === "B" && activeTab === "server") {
      scrollOffset = Math.max(0, scrollOffset - 1);
      renderFrame();
    }
    return { consumedLength: 3, incomplete: false };
  }

  // CSI sequences: ESC [ ...
  if (next === "[") {
    if (remaining === 2) {
      return { consumedLength: 0, incomplete: true };
    }

    const third: string = buf[offset + 2] ?? "";

    // SGR mouse: ESC [ < Cb ; Cx ; Cy M/m
    if (third === "<") {
      const mouseEv: ParsedMouseEvent | null = parseSgrMouse(buf, offset);
      if (mouseEv) {
        handleMouseEvent(mouseEv);
        return { consumedLength: mouseEv.consumedLength, incomplete: false };
      }
      if (remaining < 24) {
        return { consumedLength: 0, incomplete: true };
      }
      return { consumedLength: 3, incomplete: false };
    }

    // Legacy X10 mouse: ESC [ M <b1><b2><b3>
    if (third === "M") {
      if (remaining < 6) {
        return { consumedLength: 0, incomplete: true };
      }
      return { consumedLength: 6, incomplete: false };
    }

    // Standard CSI sequence ending in 0x40..0x7e
    let idx: number = offset + 2;
    while (idx < buf.length) {
      const code: number = buf.charCodeAt(idx);
      idx += 1;
      if (code >= 0x40 && code <= 0x7e) {
        const seq: string = buf.slice(offset + 2, idx);
        if (seq === "D") {
          switchTab("web");
        } else if (seq === "C") {
          switchTab("server");
        } else if (seq === "A" && activeTab === "server") {
          scrollOffset += 1;
          renderFrame();
        } else if (seq === "B" && activeTab === "server") {
          scrollOffset = Math.max(0, scrollOffset - 1);
          renderFrame();
        } else if (seq === "5~" && activeTab === "server") {
          scrollOffset += 5;
          renderFrame();
        } else if (seq === "6~" && activeTab === "server") {
          scrollOffset = Math.max(0, scrollOffset - 5);
          renderFrame();
        }
        return { consumedLength: idx - offset, incomplete: false };
      }
      if (idx - offset > 32) {
        return { consumedLength: idx - offset, incomplete: false };
      }
    }
    return { consumedLength: 0, incomplete: true };
  }

  return { consumedLength: 2, incomplete: false };
}

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
      const parsed: EscapeParseResult = parseEscapeSequence(combined, i);
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
  output.write("\x1b[?1049h\x1b[3J\x1b[2J\x1b[H\x1b[?1000h\x1b[?1006h");
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
    output.write("\x1b[3J");
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
