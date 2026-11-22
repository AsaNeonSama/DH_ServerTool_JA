import type { TerminalTabId } from "../types";

export interface ParsedMouseEvent {
  readonly button: number;
  readonly col: number;
  readonly row: number;
  readonly pressed: boolean;
  readonly consumedLength: number;
}

export interface EscapeParseResult {
  readonly consumedLength: number;
  readonly incomplete: boolean;
}

export interface EscapeActionCallbacks {
  readonly onSwitchTab: (tab: TerminalTabId) => void;
  readonly onScrollDelta: (delta: number) => void;
  readonly onScrollJump: (target: "top" | "bottom") => void;
  readonly onMouseEvent: (event: ParsedMouseEvent) => void;
}

const ESC_CODE: number = 0x1b;

export function parseSgrMouse(buf: string, offset: number): ParsedMouseEvent | null {
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

export function parseEscapeSequence(
  buf: string,
  offset: number,
  callbacks: EscapeActionCallbacks
): EscapeParseResult {
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
      callbacks.onSwitchTab("web");
    } else if (finalCh === "C") {
      callbacks.onSwitchTab("server");
    } else if (finalCh === "A") {
      callbacks.onScrollDelta(1);
    } else if (finalCh === "B") {
      callbacks.onScrollDelta(-1);
    } else if (finalCh === "H") {
      callbacks.onScrollJump("top");
    } else if (finalCh === "F") {
      callbacks.onScrollJump("bottom");
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
        callbacks.onMouseEvent(mouseEv);
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
          callbacks.onSwitchTab("web");
        } else if (seq === "C") {
          callbacks.onSwitchTab("server");
        } else if (seq === "A") {
          callbacks.onScrollDelta(1);
        } else if (seq === "B") {
          callbacks.onScrollDelta(-1);
        } else if (seq === "5~") {
          callbacks.onScrollDelta(5);
        } else if (seq === "6~") {
          callbacks.onScrollDelta(-5);
        } else if (seq === "H" || seq === "1~") {
          callbacks.onScrollJump("top");
        } else if (seq === "F" || seq === "4~") {
          callbacks.onScrollJump("bottom");
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
