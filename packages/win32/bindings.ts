import { dlopen, FFIType } from "bun:ffi";
import { INVALID_HANDLE_VALUE } from "../constants";
import type { IphlpapiLibrary, Kernel32Library, User32Library } from "../types";

export const kernel32: Kernel32Library = dlopen("kernel32.dll", {
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
  CreateJobObjectW: {
    args: [FFIType.ptr, FFIType.ptr],
    returns: FFIType.u64,
  },
  SetInformationJobObject: {
    args: [FFIType.u64, FFIType.u32, FFIType.ptr, FFIType.u32],
    returns: FFIType.i32,
  },
  AssignProcessToJobObject: {
    args: [FFIType.u64, FFIType.u64],
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

export const user32: User32Library = dlopen("user32.dll", {
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

export const iphlpapi: IphlpapiLibrary = dlopen("iphlpapi.dll", {
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

export function isValidWin32Pid(pid: number): boolean {
  return Number.isInteger(pid) && pid > 0 && pid <= 0xffffffff;
}

export function isValidWin32Handle(handle: bigint): boolean {
  return handle !== 0n && BigInt.asIntN(64, handle) !== INVALID_HANDLE_VALUE;
}
