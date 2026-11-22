import {
  C,
  MAX_WEB_LOG_LINES,
  TAB_ACTIVE_STYLE,
  TAB_BUTTON_STYLE,
  TAB_INACTIVE_STYLE,
} from "../constants";
import { TERMINAL_MESSAGES } from "../messages";
import type {
  HitRegion,
  TerminalTabId,
  WebConsoleDisplayInfo,
} from "../types";
import { countVisualRows, getDisplayWidth } from "./display";

export const TAB_HEADER_ROWS: number = 2;

export interface CopyFeedbackState {
  readonly text: string;
  readonly isError: boolean;
}

export interface TabBarOptions {
  readonly termCols: number;
  readonly activeTab: TerminalTabId;
  readonly webConsoleInfo: WebConsoleDisplayInfo | null;
  readonly serverScrollOffset: number;
}

export interface TabBarLayout {
  readonly row1: string;
  readonly row2: string;
  readonly tab1Hit: HitRegion;
  readonly tab2Hit: HitRegion;
}

export interface WebTabLayoutOptions {
  readonly termCols: number;
  readonly webConsoleInfo: WebConsoleDisplayInfo | null;
  readonly copyFeedback: CopyFeedbackState | null;
  readonly hasPendingServerAction: boolean;
}

export interface WebTabLayout {
  readonly lines: string[];
  readonly publicBtnHit: HitRegion | null;
  readonly localBtnHit: HitRegion | null;
}

export interface WebLogBarOptions {
  readonly termCols: number;
  readonly totalLines: number;
  readonly scrollPos: number;
}

export function buildTabBar(options: TabBarOptions): TabBarLayout {
  const { termCols, activeTab, webConsoleInfo, serverScrollOffset }: TabBarOptions =
    options;
  const portSuffix: string = webConsoleInfo ? ` (${webConsoleInfo.port})` : "";
  const tab1Text: string = TERMINAL_MESSAGES.tab1Title(portSuffix);
  const tab2Text: string = TERMINAL_MESSAGES.tab2Title;
  const sepText: string = " │ ";
  const hintText: string = TERMINAL_MESSAGES.tabSwitchHint;

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
  if (activeTab === "server" && serverScrollOffset > 0) {
    const scrollBadge: string =
      TERMINAL_MESSAGES.serverScrollBadge(serverScrollOffset);
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

export function buildWebTabLayout(options: WebTabLayoutOptions): WebTabLayout {
  const {
    termCols,
    webConsoleInfo,
    copyFeedback,
    hasPendingServerAction,
  }: WebTabLayoutOptions = options;
  if (!webConsoleInfo) {
    return { lines: [], publicBtnHit: null, localBtnHit: null };
  }

  const broadcastText: string = webConsoleInfo.autoBroadcast
    ? TERMINAL_MESSAGES.toggleOn
    : TERMINAL_MESSAGES.toggleOff;
  const prefixLines: string[] = [
    `${C.LIGHT_CYAN} ${TERMINAL_MESSAGES.webSettingsHeader}`,
    `${C.LIGHT_WHITE} * ${TERMINAL_MESSAGES.webTcpPortLabel} ${C.LIGHT_GREEN}${webConsoleInfo.port} ${C.LIGHT_WHITE}${TERMINAL_MESSAGES.webAutoBroadcastLabel} ${C.LIGHT_GREEN}${broadcastText}`,
    "",
    `${C.LIGHT_CYAN} ${TERMINAL_MESSAGES.webStartedHeader}`,
    `${C.LIGHT_BLACK} * ${TERMINAL_MESSAGES.publicUrlLabel} ${webConsoleInfo.publicUrl}`,
    `${C.LIGHT_BLACK} * ${TERMINAL_MESSAGES.localUrlLabel} ${webConsoleInfo.localUrl}`,
    `${C.LIGHT_YELLOW} * ${TERMINAL_MESSAGES.pskNoticeLead}${C.LIGHT_WHITE}${TERMINAL_MESSAGES.pskNoticeBody}${C.LIGHT_MAGENTA}${TERMINAL_MESSAGES.pskNoticeWarn}`,
    `${C.LIGHT_BLACK} * ${TERMINAL_MESSAGES.internalBridgePortSummary(webConsoleInfo.consolePatchPort, broadcastText)}`,
    `${C.LIGHT_CYAN} ${TERMINAL_MESSAGES.webStartedFooter}`,
    "",
  ];

  let visualRowCursor: number = TAB_HEADER_ROWS + 1;
  for (const line of prefixLines) {
    visualRowCursor += countVisualRows(line, termCols);
  }

  const pubBtnLabel: string = TERMINAL_MESSAGES.copyPublicBtnLabel;
  const locBtnLabel: string = TERMINAL_MESSAGES.copyLocalBtnLabel;
  const leftPad: string = " ";
  const gap: string = "  ";

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
  suffixLines.push(`${C.LIGHT_BLACK} * ${TERMINAL_MESSAGES.copyGuideHint}`);
  if (hasPendingServerAction) {
    suffixLines.push(
      `${C.LIGHT_YELLOW} * ${TERMINAL_MESSAGES.serverPendingInputHint}`
    );
  } else {
    suffixLines.push(`${C.LIGHT_BLACK} * ${TERMINAL_MESSAGES.switchToServerHint}`);
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

export function buildWebLogBar(options: WebLogBarOptions): string {
  const { termCols, totalLines, scrollPos }: WebLogBarOptions = options;
  const leftText: string = TERMINAL_MESSAGES.webLogBarTitle(
    totalLines,
    MAX_WEB_LOG_LINES
  );
  const rightText: string =
    scrollPos > 0
      ? TERMINAL_MESSAGES.webLogBarScrolled(scrollPos)
      : TERMINAL_MESSAGES.webLogBarControlsHint;
  const rightColor: string = scrollPos > 0 ? C.LIGHT_YELLOW : C.LIGHT_BLACK;

  const leftLead: number = 2;
  const rightTrail: number = 2;
  const leftWidth: number = getDisplayWidth(leftText);
  const rightWidth: number = getDisplayWidth(rightText);

  if (leftLead + leftWidth + 2 + rightWidth + rightTrail <= termCols) {
    const midDashes: number =
      termCols - leftLead - leftWidth - rightWidth - rightTrail;
    return (
      `${C.LIGHT_BLACK}${"─".repeat(leftLead)}` +
      `${C.LIGHT_CYAN}${leftText}` +
      `${C.LIGHT_BLACK}${"─".repeat(midDashes)}` +
      `${rightColor}${rightText}` +
      `${C.LIGHT_BLACK}${"─".repeat(rightTrail)}${C.RESET}`
    );
  }

  if (leftLead + leftWidth + rightTrail <= termCols) {
    const rightDashes: number = termCols - leftLead - leftWidth;
    return (
      `${C.LIGHT_BLACK}${"─".repeat(leftLead)}` +
      `${C.LIGHT_CYAN}${leftText}` +
      `${C.LIGHT_BLACK}${"─".repeat(rightDashes)}${C.RESET}`
    );
  }

  return `${C.LIGHT_BLACK}${"─".repeat(termCols)}${C.RESET}`;
}
