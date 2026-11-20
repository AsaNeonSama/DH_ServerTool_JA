import { dlopen, FFIType, ptr, toArrayBuffer } from "bun:ffi";
import type { Pointer } from "bun:ffi";
import {
  AF_INET,
  AF_INET6,
  CF_UNICODETEXT,
  CP_UTF8,
  ENABLE_EXTENDED_FLAGS,
  ENABLE_MOUSE_INPUT,
  ENABLE_QUICK_EDIT_MODE,
  ENABLE_VIRTUAL_TERMINAL_INPUT,
  ENABLE_VIRTUAL_TERMINAL_PROCESSING,
  ENABLE_WINDOW_INPUT,
  ERROR_INSUFFICIENT_BUFFER,
  GMEM_MOVEABLE,
  INVALID_HANDLE_VALUE,
  IP_TABLE_EXTRA_BUFFER_SIZE,
  IP_TABLE_MIN_BUFFER_SIZE,
  PE32W_EXEFILE_MAX_BYTES,
  PE32W_EXEFILE_OFFSET,
  PE32W_PID_OFFSET,
  PE32W_PPID_OFFSET,
  PROCESS_QUERY_LIMITED_INFORMATION,
  PROCESS_TERMINATE,
  PROCESSENTRY32W_SIZE,
  STD_INPUT_HANDLE,
  STD_OUTPUT_HANDLE,
  STILL_ACTIVE,
  TCP_TABLE_OWNER_PID_LISTENER,
  TCP_V4_PORT_OFFSET,
  TCP_V4_ROW_SIZE,
  TH32CS_SNAPPROCESS,
  UDP_TABLE_OWNER_PID,
  UDP_V4_PID_OFFSET,
  UDP_V4_PORT_OFFSET,
  UDP_V4_ROW_SIZE,
  UDP_V6_PID_OFFSET,
  UDP_V6_PORT_OFFSET,
  UDP_V6_ROW_SIZE,
} from "./constants";
import type {
  IphlpapiLibrary,
  Kernel32Library,
  ProcessInfo,
  UdpLookupResult,
  UdpTableQuery,
  User32Library,
} from "./types";

const kernel32: Kernel32Library = dlopen("kernel32.dll", {
  SetConsoleTitleW: {
    args: [FFIType.ptr],
    returns: FFIType.i32,
  },
  SetConsoleOutputCP: {
    args: [FFIType.u32],
    returns: FFIType.i32,
  },
  SetConsoleCP: {
    args: [FFIType.u32],
    returns: FFIType.i32,
  },
  GetStdHandle: {
    args: [FFIType.i32],
    returns: FFIType.u64,
  },
  GetConsoleMode: {
    args: [FFIType.u64, FFIType.ptr],
    returns: FFIType.i32,
  },
  SetConsoleMode: {
    args: [FFIType.u64, FFIType.u32],
    returns: FFIType.i32,
  },
  CreateToolhelp32Snapshot: {
    args: [FFIType.u32, FFIType.u32],
    returns: FFIType.i64,
  },
  Process32FirstW: {
    args: [FFIType.i64, FFIType.ptr],
    returns: FFIType.i32,
  },
  Process32NextW: {
    args: [FFIType.i64, FFIType.ptr],
    returns: FFIType.i32,
  },
  OpenProcess: {
    args: [FFIType.u32, FFIType.i32, FFIType.u32],
    returns: FFIType.u64,
  },
  GetExitCodeProcess: {
    args: [FFIType.u64, FFIType.ptr],
    returns: FFIType.i32,
  },
  TerminateProcess: {
    args: [FFIType.u64, FFIType.u32],
    returns: FFIType.i32,
  },
  CloseHandle: {
    args: [FFIType.u64],
    returns: FFIType.i32,
  },
  GlobalAlloc: {
    args: [FFIType.u32, FFIType.u64],
    returns: FFIType.u64,
  },
  GlobalLock: {
    args: [FFIType.u64],
    returns: FFIType.ptr,
  },
  GlobalUnlock: {
    args: [FFIType.u64],
    returns: FFIType.i32,
  },
  GlobalFree: {
    args: [FFIType.u64],
    returns: FFIType.u64,
  },
});

const user32: User32Library = dlopen("user32.dll", {
  OpenClipboard: {
    args: [FFIType.u64],
    returns: FFIType.i32,
  },
  EmptyClipboard: {
    args: [],
    returns: FFIType.i32,
  },
  SetClipboardData: {
    args: [FFIType.u32, FFIType.u64],
    returns: FFIType.u64,
  },
  CloseClipboard: {
    args: [],
    returns: FFIType.i32,
  },
});

const iphlpapi: IphlpapiLibrary = dlopen("iphlpapi.dll", {
  GetExtendedUdpTable: {
    args: [
      FFIType.ptr,
      FFIType.ptr,
      FFIType.i32,
      FFIType.u32,
      FFIType.u32,
      FFIType.u32,
    ],
    returns: FFIType.u32,
  },
  GetExtendedTcpTable: {
    args: [
      FFIType.ptr,
      FFIType.ptr,
      FFIType.i32,
      FFIType.u32,
      FFIType.u32,
      FFIType.u32,
    ],
    returns: FFIType.u32,
  },
});

export function initWindowsConsole(title: string): void {
  try {
    kernel32.symbols.SetConsoleOutputCP(CP_UTF8);
    kernel32.symbols.SetConsoleCP(CP_UTF8);

    const titleBuf: Buffer = Buffer.from(`${title}\0`, "utf16le");
    kernel32.symbols.SetConsoleTitleW(ptr(titleBuf));

    const hOut: bigint = kernel32.symbols.GetStdHandle(STD_OUTPUT_HANDLE);
    if (hOut !== 0n && BigInt.asIntN(64, hOut) !== INVALID_HANDLE_VALUE) {
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
    if (hIn === 0n || BigInt.asIntN(64, hIn) === INVALID_HANDLE_VALUE) {
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
    if (hIn === 0n || BigInt.asIntN(64, hIn) === INVALID_HANDLE_VALUE) {
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
    if (hIn === 0n || BigInt.asIntN(64, hIn) === INVALID_HANDLE_VALUE) {
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
    if (locked === null || typeof locked === "bigint") {
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
      user32.symbols.EmptyClipboard();
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

export function listProcesses(): ProcessInfo[] {
  const results: ProcessInfo[] = [];
  const snap: bigint = kernel32.symbols.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
  if (snap === INVALID_HANDLE_VALUE) {
    return results;
  }

  try {
    const entry: Uint8Array = new Uint8Array(PROCESSENTRY32W_SIZE);
    const view: DataView = new DataView(entry.buffer);
    view.setUint32(0, PROCESSENTRY32W_SIZE, true);

    if (kernel32.symbols.Process32FirstW(snap, ptr(entry)) === 0) {
      return results;
    }

    const decoder: TextDecoder = new TextDecoder("utf-16le");
    do {
      const pid: number = view.getUint32(PE32W_PID_OFFSET, true);
      const parentPid: number = view.getUint32(PE32W_PPID_OFFSET, true);
      const rawName: Uint8Array = entry.subarray(
        PE32W_EXEFILE_OFFSET,
        PE32W_EXEFILE_OFFSET + PE32W_EXEFILE_MAX_BYTES
      );
      let nullPos: number = PE32W_EXEFILE_MAX_BYTES;
      for (let i: number = 0; i < PE32W_EXEFILE_MAX_BYTES; i += 2) {
        if (rawName[i] === 0 && rawName[i + 1] === 0) {
          nullPos = i;
          break;
        }
      }
      const name: string = decoder.decode(rawName.subarray(0, nullPos));
      results.push({ pid, parentPid, name });
      view.setUint32(0, PROCESSENTRY32W_SIZE, true);
    } while (kernel32.symbols.Process32NextW(snap, ptr(entry)) !== 0);
  } finally {
    kernel32.symbols.CloseHandle(BigInt.asUintN(64, snap));
  }

  return results;
}

export function listProcessesByName(exeName: string): ProcessInfo[] {
  const target: string = exeName.toLowerCase();
  return listProcesses().filter((proc: ProcessInfo): boolean => proc.name.toLowerCase() === target);
}

export function isProcessAlive(pid: number): boolean {
  if (pid <= 0) return false;
  const handle: bigint = kernel32.symbols.OpenProcess(
    PROCESS_QUERY_LIMITED_INFORMATION,
    0,
    pid
  );
  if (handle === 0n) {
    return false;
  }
  try {
    const exitCodeBuf: Uint32Array = new Uint32Array(1);
    const ok: number = kernel32.symbols.GetExitCodeProcess(handle, ptr(exitCodeBuf));
    if (ok === 0) return false;
    return exitCodeBuf[0] === STILL_ACTIVE;
  } finally {
    kernel32.symbols.CloseHandle(handle);
  }
}

export function terminateProcessByPid(pid: number): boolean {
  if (pid <= 0) return false;
  const handle: bigint = kernel32.symbols.OpenProcess(PROCESS_TERMINATE, 0, pid);
  if (handle === 0n) return false;
  try {
    return kernel32.symbols.TerminateProcess(handle, 0) !== 0;
  } finally {
    kernel32.symbols.CloseHandle(handle);
  }
}

function ntohsFromDword(dword: number): number {
  return ((dword & 0xff) << 8) | ((dword >> 8) & 0xff);
}

function queryUdpTable(query: UdpTableQuery): UdpLookupResult {
  const sizeBuf: Uint32Array = new Uint32Array(1);
  let status: number = iphlpapi.symbols.GetExtendedUdpTable(
    null,
    ptr(sizeBuf),
    0,
    query.addressFamily,
    UDP_TABLE_OWNER_PID,
    0
  );
  if (status !== ERROR_INSUFFICIENT_BUFFER && status !== 0) {
    return { available: false, pid: null };
  }
  const allocSize: number =
    Math.max(sizeBuf[0] ?? 0, IP_TABLE_MIN_BUFFER_SIZE) + IP_TABLE_EXTRA_BUFFER_SIZE;
  sizeBuf[0] = allocSize;
  const tableBuf: Uint8Array = new Uint8Array(allocSize);
  status = iphlpapi.symbols.GetExtendedUdpTable(
    ptr(tableBuf),
    ptr(sizeBuf),
    0,
    query.addressFamily,
    UDP_TABLE_OWNER_PID,
    0
  );
  if (status !== 0) {
    return { available: false, pid: null };
  }

  const view: DataView = new DataView(tableBuf.buffer);
  const count: number = view.getUint32(0, true);
  for (let i: number = 0; i < count; i++) {
    const base: number = 4 + i * query.rowSize;
    if (base + query.rowSize > tableBuf.byteLength) break;
    const rawPort: number = view.getUint32(base + query.portOffset, true);
    if (ntohsFromDword(rawPort) === query.targetPort) {
      const ownerPid: number = view.getUint32(base + query.pidOffset, true);
      if (query.expectedPid === undefined || ownerPid === query.expectedPid) {
        return { available: true, pid: ownerPid };
      }
    }
  }
  return { available: true, pid: null };
}

export function findUdpListenerPid(
  port: number,
  expectedPid?: number
): UdpLookupResult {
  const v4: UdpLookupResult = queryUdpTable({
    addressFamily: AF_INET,
    rowSize: UDP_V4_ROW_SIZE,
    portOffset: UDP_V4_PORT_OFFSET,
    pidOffset: UDP_V4_PID_OFFSET,
    targetPort: port,
    expectedPid,
  });
  if (v4.pid !== null) return v4;

  const v6: UdpLookupResult = queryUdpTable({
    addressFamily: AF_INET6,
    rowSize: UDP_V6_ROW_SIZE,
    portOffset: UDP_V6_PORT_OFFSET,
    pidOffset: UDP_V6_PID_OFFSET,
    targetPort: port,
    expectedPid,
  });
  if (v6.pid !== null) return v6;

  return { available: v4.available || v6.available, pid: null };
}

export function isTcpPortListening(port: number): boolean {
  const sizeBuf: Uint32Array = new Uint32Array(1);
  let status: number = iphlpapi.symbols.GetExtendedTcpTable(
    null,
    ptr(sizeBuf),
    0,
    AF_INET,
    TCP_TABLE_OWNER_PID_LISTENER,
    0
  );
  if (status !== ERROR_INSUFFICIENT_BUFFER && status !== 0) {
    return false;
  }
  const allocSize: number =
    Math.max(sizeBuf[0] ?? 0, IP_TABLE_MIN_BUFFER_SIZE) + IP_TABLE_EXTRA_BUFFER_SIZE;
  sizeBuf[0] = allocSize;
  const tableBuf: Uint8Array = new Uint8Array(allocSize);
  status = iphlpapi.symbols.GetExtendedTcpTable(
    ptr(tableBuf),
    ptr(sizeBuf),
    0,
    AF_INET,
    TCP_TABLE_OWNER_PID_LISTENER,
    0
  );
  if (status !== 0) {
    return false;
  }

  const view: DataView = new DataView(tableBuf.buffer);
  const count: number = view.getUint32(0, true);
  for (let i: number = 0; i < count; i++) {
    const base: number = 4 + i * TCP_V4_ROW_SIZE;
    if (base + TCP_V4_ROW_SIZE > tableBuf.byteLength) break;
    const rawPort: number = view.getUint32(base + TCP_V4_PORT_OFFSET, true);
    if (ntohsFromDword(rawPort) === port) {
      return true;
    }
  }
  return false;
}
