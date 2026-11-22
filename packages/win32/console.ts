import { ptr, toArrayBuffer } from "bun:ffi";
import type { Pointer } from "bun:ffi";
import {
  CF_UNICODETEXT,
  CP_UTF8,
  ENABLE_EXTENDED_FLAGS,
  ENABLE_MOUSE_INPUT,
  ENABLE_QUICK_EDIT_MODE,
  ENABLE_VIRTUAL_TERMINAL_INPUT,
  ENABLE_VIRTUAL_TERMINAL_PROCESSING,
  ENABLE_WINDOW_INPUT,
  GMEM_MOVEABLE,
  STD_INPUT_HANDLE,
  STD_OUTPUT_HANDLE,
} from "../constants";
import { isValidWin32Handle, kernel32, user32 } from "./bindings";

export function initWindowsConsole(title: string): void {
  try {
    kernel32.symbols.SetConsoleOutputCP(CP_UTF8);
    kernel32.symbols.SetConsoleCP(CP_UTF8);

    const titleBuf: Buffer = Buffer.from(`${title}\0`, "utf16le");
    kernel32.symbols.SetConsoleTitleW(ptr(titleBuf));

    const hOut: bigint = kernel32.symbols.GetStdHandle(STD_OUTPUT_HANDLE);
    if (isValidWin32Handle(hOut)) {
      const modeBuf: Uint32Array = new Uint32Array(1);
      if (kernel32.symbols.GetConsoleMode(hOut, ptr(modeBuf)) !== 0) {
        const nextMode: number = (modeBuf[0] ?? 0) | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
        kernel32.symbols.SetConsoleMode(hOut, nextMode);
      }
    }
  } catch {
    // Non-fatal when stdout is redirected
  }
}

export function getWindowsInputMode(): number | null {
  try {
    const hIn: bigint = kernel32.symbols.GetStdHandle(STD_INPUT_HANDLE);
    if (!isValidWin32Handle(hIn)) {
      return null;
    }
    const modeBuf: Uint32Array = new Uint32Array(1);
    if (kernel32.symbols.GetConsoleMode(hIn, ptr(modeBuf)) === 0) {
      return null;
    }
    return modeBuf[0] ?? null;
  } catch {
    return null;
  }
}

export function enableWindowsTabMouseInput(): void {
  try {
    const hIn: bigint = kernel32.symbols.GetStdHandle(STD_INPUT_HANDLE);
    if (!isValidWin32Handle(hIn)) {
      return;
    }
    const modeBuf: Uint32Array = new Uint32Array(1);
    if (kernel32.symbols.GetConsoleMode(hIn, ptr(modeBuf)) === 0) {
      return;
    }
    const current: number = modeBuf[0] ?? 0;
    const enableMask: number =
      ENABLE_MOUSE_INPUT
      | ENABLE_EXTENDED_FLAGS
      | ENABLE_VIRTUAL_TERMINAL_INPUT
      | ENABLE_WINDOW_INPUT;
    const nextMode: number = ((current | enableMask) & ~ENABLE_QUICK_EDIT_MODE) >>> 0;
    kernel32.symbols.SetConsoleMode(hIn, nextMode);
  } catch {
    // Non-fatal when stdin is redirected
  }
}

export function restoreWindowsInputMode(savedMode: number | null): void {
  if (savedMode === null) return;
  try {
    const hIn: bigint = kernel32.symbols.GetStdHandle(STD_INPUT_HANDLE);
    if (!isValidWin32Handle(hIn)) {
      return;
    }
    kernel32.symbols.SetConsoleMode(hIn, savedMode);
  } catch {
    // Ignore restore failure
  }
}

export function copyTextToClipboard(text: string): boolean {
  try {
    const utf16: Buffer = Buffer.from(`${text}\0`, "utf16le");
    const hMem: bigint = kernel32.symbols.GlobalAlloc(
      GMEM_MOVEABLE,
      BigInt(utf16.byteLength)
    );
    if (hMem === 0n) {
      return false;
    }
    const locked: Pointer | bigint | null = kernel32.symbols.GlobalLock(hMem);
    if (!locked || typeof locked === "bigint") {
      kernel32.symbols.GlobalFree(hMem);
      return false;
    }
    try {
      const dest: Uint8Array = new Uint8Array(
        toArrayBuffer(locked, 0, utf16.byteLength)
      );
      dest.set(utf16);
    } finally {
      kernel32.symbols.GlobalUnlock(hMem);
    }

    if (user32.symbols.OpenClipboard(0n) === 0) {
      kernel32.symbols.GlobalFree(hMem);
      return false;
    }
    try {
      if (user32.symbols.EmptyClipboard() === 0) {
        kernel32.symbols.GlobalFree(hMem);
        return false;
      }
      const result: bigint = user32.symbols.SetClipboardData(CF_UNICODETEXT, hMem);
      if (result === 0n) {
        kernel32.symbols.GlobalFree(hMem);
        return false;
      }
      return true;
    } finally {
      user32.symbols.CloseClipboard();
    }
  } catch {
    return false;
  }
}
