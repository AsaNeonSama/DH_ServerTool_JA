import { ptr } from "bun:ffi";
import {
  AF_INET,
  AF_INET6,
  ERROR_INSUFFICIENT_BUFFER,
  IP_TABLE_EXTRA_BUFFER_SIZE,
  IP_TABLE_MIN_BUFFER_SIZE,
  TCP_TABLE_OWNER_PID_LISTENER,
  TCP_V4_PORT_OFFSET,
  TCP_V4_ROW_SIZE,
  UDP_TABLE_OWNER_PID,
  UDP_V4_PID_OFFSET,
  UDP_V4_PORT_OFFSET,
  UDP_V4_ROW_SIZE,
  UDP_V6_PID_OFFSET,
  UDP_V6_PORT_OFFSET,
  UDP_V6_ROW_SIZE,
} from "../constants";
import type { UdpLookupResult, UdpTableQuery } from "../types";
import { iphlpapi } from "./bindings";

function ntohsFromDword(dword: number): number {
  return ((dword & 0xff) << 8) | ((dword >> 8) & 0xff);
}

function queryUdpTable(query: UdpTableQuery): UdpLookupResult {
  try {
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
    let allocSize: number =
      Math.max(sizeBuf[0] ?? 0, IP_TABLE_MIN_BUFFER_SIZE) + IP_TABLE_EXTRA_BUFFER_SIZE;
    let tableBuf: Uint8Array = new Uint8Array(allocSize);
    for (let attempt: number = 0; attempt < 3; attempt++) {
      sizeBuf[0] = allocSize;
      status = iphlpapi.symbols.GetExtendedUdpTable(
        ptr(tableBuf),
        ptr(sizeBuf),
        0,
        query.addressFamily,
        UDP_TABLE_OWNER_PID,
        0
      );
      if (status !== ERROR_INSUFFICIENT_BUFFER) {
        break;
      }
      allocSize = Math.max(sizeBuf[0] ?? 0, allocSize) + IP_TABLE_EXTRA_BUFFER_SIZE;
      tableBuf = new Uint8Array(allocSize);
    }
    if (status !== 0 || tableBuf.byteLength < 4) {
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
  } catch {
    return { available: false, pid: null };
  }
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
  try {
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
    let allocSize: number =
      Math.max(sizeBuf[0] ?? 0, IP_TABLE_MIN_BUFFER_SIZE) + IP_TABLE_EXTRA_BUFFER_SIZE;
    let tableBuf: Uint8Array = new Uint8Array(allocSize);
    for (let attempt: number = 0; attempt < 3; attempt++) {
      sizeBuf[0] = allocSize;
      status = iphlpapi.symbols.GetExtendedTcpTable(
        ptr(tableBuf),
        ptr(sizeBuf),
        0,
        AF_INET,
        TCP_TABLE_OWNER_PID_LISTENER,
        0
      );
      if (status !== ERROR_INSUFFICIENT_BUFFER) {
        break;
      }
      allocSize = Math.max(sizeBuf[0] ?? 0, allocSize) + IP_TABLE_EXTRA_BUFFER_SIZE;
      tableBuf = new Uint8Array(allocSize);
    }
    if (status !== 0 || tableBuf.byteLength < 4) {
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
  } catch {
    return false;
  }
}
