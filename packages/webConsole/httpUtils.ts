import { PLAYER_NAME_DICT } from "../constants";
import type { ApiResultPayload } from "../types";

export const MAX_ENCRYPTED_BODY_BYTES: number = 65536;

export function parseCookies(header: string | null): Record<string, string> {
  const out: Record<string, string> = {};
  if (!header) return out;
  const parts: string[] = header.split(";");
  for (const part of parts) {
    const eq: number = part.indexOf("=");
    if (eq === -1) continue;
    const key: string = part.slice(0, eq).trim();
    const val: string = part.slice(eq + 1).trim();
    if (!key) continue;
    try {
      out[key] = decodeURIComponent(val);
    } catch {
      out[key] = val;
    }
  }
  return out;
}

export async function readBoundedRequestText(
  req: Request,
  maxBytes: number = MAX_ENCRYPTED_BODY_BYTES
): Promise<string | null> {
  const contentLenHeader: string | null = req.headers.get("content-length");
  if (contentLenHeader !== null) {
    const declaredLen: number = Number(contentLenHeader);
    if (Number.isFinite(declaredLen) && declaredLen > maxBytes) {
      return null;
    }
  }
  try {
    const rawBuf: ArrayBuffer = await req.arrayBuffer();
    if (rawBuf.byteLength > maxBytes) {
      return null;
    }
    return new TextDecoder("utf-8").decode(rawBuf).trim();
  } catch {
    return null;
  }
}

export function toStringValue(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return "";
}

export function redirectTo(location: string): Response {
  return new Response(null, {
    status: 302,
    headers: { Location: location },
  });
}

export function redirectToUnAuth(): Response {
  return redirectTo("/dh-console/un-auth");
}

export function apiResult(status: number, msg: string): ApiResultPayload {
  return { status, msg };
}

export function roleDisplay(name: string): string {
  return PLAYER_NAME_DICT[name] ?? name;
}

export function parseJsonObjectString(
  text: string
): Record<string, unknown> | null {
  try {
    const parsed: unknown = JSON.parse(text);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      return null;
    }
    return parsed as Record<string, unknown>;
  } catch {
    return null;
  }
}
