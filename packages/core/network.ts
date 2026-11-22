import { DEFAULT_PUBLIC_IP_TIMEOUT_MS, PUBLIC_IP_ENDPOINT } from "../constants";
import type { PublicIpResult } from "../types";

export async function fetchPublicIp(
  timeoutMs: number = DEFAULT_PUBLIC_IP_TIMEOUT_MS
): Promise<PublicIpResult> {
  const controller: AbortController = new AbortController();
  const timer: ReturnType<typeof setTimeout> = setTimeout((): void => {
    controller.abort();
  }, timeoutMs);

  try {
    const res: Response = await fetch(PUBLIC_IP_ENDPOINT, {
      signal: controller.signal,
    });
    if (!res.ok) {
      return { ip: null, error: `HTTP ${res.status}` };
    }
    const text: string = (await res.text()).trim();
    if (!text || text.length > 64 || !/^[0-9a-fA-F.:]+$/.test(text)) {
      return { ip: null, error: "Invalid IP response" };
    }
    return { ip: text };
  } catch (err: unknown) {
    const msg: string = err instanceof Error ? err.message : String(err);
    return { ip: null, error: msg };
  } finally {
    clearTimeout(timer);
  }
}
