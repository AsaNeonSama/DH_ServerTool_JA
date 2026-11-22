import { ptr } from "bun:ffi";
import {
  INVALID_HANDLE_VALUE,
  JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
  JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
  JOBOBJECT_EXTENDED_LIMIT_INFO_SIZE,
  JOBOBJECT_LIMIT_FLAGS_OFFSET,
  PE32W_EXEFILE_MAX_BYTES,
  PE32W_EXEFILE_OFFSET,
  PE32W_PID_OFFSET,
  PE32W_PPID_OFFSET,
  PROCESS_QUERY_LIMITED_INFORMATION,
  PROCESS_SET_QUOTA,
  PROCESS_TERMINATE,
  PROCESSENTRY32W_SIZE,
  STILL_ACTIVE,
  TH32CS_SNAPPROCESS,
} from "../constants";
import type { ProcessInfo } from "../types";
import { isValidWin32Handle, isValidWin32Pid, kernel32 } from "./bindings";

export function listProcesses(): ProcessInfo[] {
  const results: ProcessInfo[] = [];
  let snap: bigint;
  try {
    snap = kernel32.symbols.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
  } catch {
    return results;
  }
  if (snap === INVALID_HANDLE_VALUE || snap === 0n) {
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
  } catch {
    // Return partial snapshot cleanly on transient Win32 error
  } finally {
    try {
      kernel32.symbols.CloseHandle(BigInt.asUintN(64, snap));
    } catch {
      // Ignore close failure
    }
  }

  return results;
}

export function listProcessesByName(exeName: string): ProcessInfo[] {
  const target: string = exeName.toLowerCase();
  return listProcesses().filter(
    (proc: ProcessInfo): boolean => proc.name.toLowerCase() === target
  );
}

export function isProcessAlive(pid: number): boolean {
  if (!isValidWin32Pid(pid)) return false;
  try {
    const handle: bigint = kernel32.symbols.OpenProcess(
      PROCESS_QUERY_LIMITED_INFORMATION,
      0,
      pid
    );
    if (!isValidWin32Handle(handle)) {
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
  } catch {
    return false;
  }
}

export function terminateProcessByPid(pid: number): boolean {
  if (!isValidWin32Pid(pid)) return false;
  try {
    const handle: bigint = kernel32.symbols.OpenProcess(PROCESS_TERMINATE, 0, pid);
    if (!isValidWin32Handle(handle)) return false;
    try {
      return kernel32.symbols.TerminateProcess(handle, 0) !== 0;
    } finally {
      kernel32.symbols.CloseHandle(handle);
    }
  } catch {
    return false;
  }
}

let launcherJobHandle: bigint | null = null;

function ensureLauncherJob(): bigint | null {
  if (launcherJobHandle !== null) {
    return launcherJobHandle;
  }
  const hJob: bigint = kernel32.symbols.CreateJobObjectW(null, null);
  if (!isValidWin32Handle(hJob)) {
    return null;
  }
  const infoBuf: Uint8Array = new Uint8Array(JOBOBJECT_EXTENDED_LIMIT_INFO_SIZE);
  const view: DataView = new DataView(infoBuf.buffer);
  view.setUint32(
    JOBOBJECT_LIMIT_FLAGS_OFFSET,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    true
  );
  const ok: number = kernel32.symbols.SetInformationJobObject(
    hJob,
    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
    ptr(infoBuf),
    JOBOBJECT_EXTENDED_LIMIT_INFO_SIZE
  );
  if (ok === 0) {
    kernel32.symbols.CloseHandle(hJob);
    return null;
  }
  launcherJobHandle = hJob;
  return hJob;
}

/**
 * Assigns a spawned server process to an anonymous Win32 Job Object configured with
 * JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE so the OS kernel immediately terminates that
 * server process whenever this launcher exits (Ctrl+C, window close X, crash, or kill).
 */
export function bindProcessToLauncherJob(pid: number): boolean {
  if (!isValidWin32Pid(pid)) return false;
  try {
    const hJob: bigint | null = ensureLauncherJob();
    if (hJob === null) return false;
    const hProc: bigint = kernel32.symbols.OpenProcess(
      PROCESS_SET_QUOTA | PROCESS_TERMINATE,
      0,
      pid
    );
    if (!isValidWin32Handle(hProc)) return false;
    try {
      return kernel32.symbols.AssignProcessToJobObject(hJob, hProc) !== 0;
    } finally {
      kernel32.symbols.CloseHandle(hProc);
    }
  } catch {
    return false;
  }
}

/**
 * Explicitly closes the launcher's Job Object handle, terminating any child
 * server processes still bound to it.
 */
export function closeLauncherJob(): void {
  if (launcherJobHandle === null) return;
  const hJob: bigint = launcherJobHandle;
  launcherJobHandle = null;
  try {
    kernel32.symbols.CloseHandle(hJob);
  } catch {
    // Ignore handle close error during shutdown
  }
}
