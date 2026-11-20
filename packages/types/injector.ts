import type { Script, Session } from "frida";
import type { PatchScript } from "./mods";

export interface SpawnAndInjectOptions {
  readonly shippingExecutable: string;
  readonly ueMapArgument: string;
  readonly port: number;
  readonly patches: readonly PatchScript[];
  readonly scriptGapMs: number;
  readonly onDetached?: (reason: string) => void;
}

export interface ActiveInjectionSession {
  readonly pid: number;
  readonly session: Session;
  readonly scripts: readonly Script[];
  readonly isDetached: () => boolean;
  readonly detachReason: () => string | null;
  readonly cleanup: () => Promise<void>;
}
