export function formatLocalTime(now: Date = new Date()): string {
  const hh: string = String(now.getHours()).padStart(2, "0");
  const mm: string = String(now.getMinutes()).padStart(2, "0");
  const ss: string = String(now.getSeconds()).padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}

export function formatLogDateTime(now: Date = new Date()): string {
  const yyyy: string = String(now.getFullYear());
  const mo: string = String(now.getMonth() + 1).padStart(2, "0");
  const dd: string = String(now.getDate()).padStart(2, "0");
  const hh: string = String(now.getHours()).padStart(2, "0");
  const mm: string = String(now.getMinutes()).padStart(2, "0");
  const ss: string = String(now.getSeconds()).padStart(2, "0");
  const ms: string = String(now.getMilliseconds()).padStart(3, "0");
  return `${yyyy}-${mo}-${dd} ${hh}:${mm}:${ss}.${ms}`;
}

export function formatSeconds(ms: number): string {
  const seconds: number = ms / 1000;
  const formatted: string = Number.isInteger(seconds)
    ? String(seconds)
    : seconds.toFixed(2).replace(/\.?0+$/, "");
  return `${formatted}s`;
}

export function sanitizeLogText(text: string, maxChars: number = 64): string {
  let clean: string = "";
  for (const ch of text) {
    const code: number = ch.codePointAt(0) ?? 0;
    if (code === 0x09 || code === 0x0a || code === 0x0d) {
      clean += " ";
      continue;
    }
    if (code < 0x20 || (code >= 0x7f && code <= 0x9f)) {
      continue;
    }
    clean += ch;
  }
  const collapsed: string = clean.replace(/\s+/g, " ").trim();
  const chars: string[] = Array.from(collapsed);
  if (chars.length <= maxChars) {
    return collapsed;
  }
  return `${chars.slice(0, Math.max(1, maxChars - 1)).join("")}…`;
}
