import type { Pointer } from "bun:ffi";

export interface ProcessInfo {
  readonly pid: number;
  readonly parentPid: number;
  readonly name: string;
}

export interface UdpLookupResult {
  readonly available: boolean;
  readonly pid: number | null;
}

export interface UdpTableQuery {
  readonly addressFamily: number;
  readonly rowSize: number;
  readonly portOffset: number;
  readonly pidOffset: number;
  readonly targetPort: number;
  readonly expectedPid?: number;
}

export interface Kernel32Symbols {
  readonly SetConsoleTitleW: (titlePtr: Pointer) => number;
  readonly SetConsoleOutputCP: (codePageId: number) => number;
  readonly SetConsoleCP: (codePageId: number) => number;
  readonly GetStdHandle: (stdHandle: number) => bigint;
  readonly GetConsoleMode: (consoleHandle: bigint, modePtr: Pointer) => number;
  readonly SetConsoleMode: (consoleHandle: bigint, mode: number) => number;
  readonly CreateToolhelp32Snapshot: (flags: number, processId: number) => bigint;
  readonly Process32FirstW: (snapshot: bigint, entryPtr: Pointer) => number;
  readonly Process32NextW: (snapshot: bigint, entryPtr: Pointer) => number;
  readonly OpenProcess: (desiredAccess: number, inheritHandle: number, processId: number) => bigint;
  readonly GetExitCodeProcess: (processHandle: bigint, exitCodePtr: Pointer) => number;
  readonly TerminateProcess: (processHandle: bigint, exitCode: number) => number;
  readonly CloseHandle: (objectHandle: bigint) => number;
  readonly GlobalAlloc: (flags: number, bytes: bigint) => bigint;
  readonly GlobalLock: (memHandle: bigint) => Pointer | bigint | null;
  readonly GlobalUnlock: (memHandle: bigint) => number;
  readonly GlobalFree: (memHandle: bigint) => bigint;
}

export interface Kernel32Library {
  readonly symbols: Kernel32Symbols;
}

export interface User32Symbols {
  readonly OpenClipboard: (ownerWindow: bigint) => number;
  readonly EmptyClipboard: () => number;
  readonly SetClipboardData: (format: number, memHandle: bigint) => bigint;
  readonly CloseClipboard: () => number;
}

export interface User32Library {
  readonly symbols: User32Symbols;
}

export type ExtendedIpTableArgs = [
  table: Pointer | null,
  sizePtr: Pointer,
  order: number,
  addressFamily: number,
  tableClass: number,
  reserved: number
];

export interface IphlpapiSymbols {
  readonly GetExtendedUdpTable: (...args: ExtendedIpTableArgs) => number;
  readonly GetExtendedTcpTable: (...args: ExtendedIpTableArgs) => number;
}

export interface IphlpapiLibrary {
  readonly symbols: IphlpapiSymbols;
}
