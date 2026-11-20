export type TerminalTabId = "web" | "server";

export type ServerKeyHandler = (key: string) => boolean;

export interface HitRegion {
  readonly row: number;
  readonly startCol: number;
  readonly endCol: number;
}
