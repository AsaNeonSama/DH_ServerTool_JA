export interface VisibleLinesOptions {
  readonly lines: readonly string[];
  readonly currentOffset: number;
  readonly availableRows: number;
  readonly termCols: number;
}

export interface VisibleLinesResult {
  readonly visibleLines: string[];
  readonly clampedOffset: number;
}

const ESC_CODE: number = 0x1b;

export function stripAnsi(text: string): string {
  let result: string = "";
  let i: number = 0;
  while (i < text.length) {
    if (text.charCodeAt(i) === ESC_CODE && text[i + 1] === "[") {
      i += 2;
      while (i < text.length) {
        const code: number = text.charCodeAt(i);
        i += 1;
        if (code >= 0x40 && code <= 0x7e) {
          break;
        }
      }
      continue;
    }
    result += text[i] ?? "";
    i += 1;
  }
  return result;
}

export function getDisplayWidth(text: string): number {
  const plain: string = stripAnsi(text);
  let width: number = 0;
  for (const ch of plain) {
    const code: number = ch.codePointAt(0) ?? 0;
    if (code < 0x20 || code === 0x7f) {
      continue;
    }
    const isWide: boolean =
      (code >= 0x1100 && code <= 0x115f) ||
      (code >= 0x2e80 && code <= 0xa4cf && code !== 0x303f) ||
      (code >= 0xac00 && code <= 0xd7a3) ||
      (code >= 0xf900 && code <= 0xfaff) ||
      (code >= 0xfe10 && code <= 0xfe19) ||
      (code >= 0xfe30 && code <= 0xfe6f) ||
      (code >= 0xff00 && code <= 0xff60) ||
      (code >= 0xffe0 && code <= 0xffe6);
    width += isWide ? 2 : 1;
  }
  return width;
}

export function padDisplayWidth(text: string, targetWidth: number): string {
  const width: number = getDisplayWidth(text);
  const pad: number = Math.max(0, targetWidth - width);
  return `${text}${" ".repeat(pad)}`;
}

export function countVisualRows(line: string, termCols: number): number {
  const width: number = getDisplayWidth(line);
  if (width === 0) return 1;
  return Math.max(1, Math.ceil(width / Math.max(1, termCols)));
}

export function truncateLineToDisplayWidth(line: string, maxWidth: number): string {
  if (maxWidth <= 0 || getDisplayWidth(line) <= maxWidth) {
    return line;
  }
  const targetWidth: number = Math.max(1, maxWidth - 1);
  let out: string = "";
  let width: number = 0;
  let i: number = 0;
  while (i < line.length) {
    if (line.charCodeAt(i) === ESC_CODE && line[i + 1] === "[") {
      const start: number = i;
      i += 2;
      while (i < line.length) {
        const code: number = line.charCodeAt(i);
        i += 1;
        if (code >= 0x40 && code <= 0x7e) {
          break;
        }
      }
      out += line.slice(start, i);
      continue;
    }
    const cp: number = line.codePointAt(i) ?? 0;
    const ch: string = String.fromCodePoint(cp);
    const chWidth: number = getDisplayWidth(ch);
    if (width + chWidth > targetWidth) {
      break;
    }
    out += ch;
    width += chWidth;
    i += ch.length;
  }
  return `${out}…`;
}

export function computeMaxScrollOffset(
  lines: readonly string[],
  availableRows: number,
  termCols: number
): number {
  let totalRows: number = 0;
  for (const line of lines) {
    totalRows += countVisualRows(line, termCols);
  }
  if (totalRows <= availableRows) {
    return 0;
  }
  let usedFromTop: number = 0;
  let fitCountFromTop: number = 0;
  for (const line of lines) {
    const r: number = countVisualRows(line, termCols);
    if (usedFromTop + r > availableRows && fitCountFromTop > 0) {
      break;
    }
    usedFromTop += r;
    fitCountFromTop += 1;
  }
  return Math.max(0, lines.length - fitCountFromTop);
}

export function selectVisibleLines(options: VisibleLinesOptions): VisibleLinesResult {
  const { lines, currentOffset, availableRows, termCols }: VisibleLinesOptions =
    options;
  if (lines.length === 0) {
    return { visibleLines: [], clampedOffset: 0 };
  }

  const maxOffset: number = computeMaxScrollOffset(lines, availableRows, termCols);
  const clampedOffset: number = Math.min(Math.max(0, currentOffset), maxOffset);

  const endIdx: number = Math.max(0, lines.length - clampedOffset);
  const selected: string[] = [];
  let usedRows: number = 0;

  for (let i: number = endIdx - 1; i >= 0; i--) {
    const line: string | undefined = lines[i];
    if (line === undefined) continue;
    const rowsNeeded: number = countVisualRows(line, termCols);
    if (usedRows + rowsNeeded > availableRows) {
      if (selected.length === 0) {
        selected.push(
          truncateLineToDisplayWidth(line, availableRows * Math.max(1, termCols))
        );
      }
      break;
    }
    selected.push(line);
    usedRows += rowsNeeded;
    if (usedRows >= availableRows) {
      break;
    }
  }

  selected.reverse();
  return { visibleLines: selected, clampedOffset };
}
