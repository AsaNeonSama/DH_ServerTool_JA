export interface AppConfig {
  readonly serverPorts: readonly [number, number];
  readonly consolePatchPort: number;
  readonly webConsolePort: number;
  readonly checkIntervalMs: number;
  readonly restartWaitSeconds: number;
  readonly injectDelayMs: number;
  readonly scriptGapMs: number;
  readonly autoBroadcastDefault: boolean;
  readonly serverExecutable: string;
  readonly shippingExecutable: string;
  readonly shippingProcessName: string;
}

export interface RawConfig {
  readonly serverPorts?: unknown;
  readonly consolePatchPort?: unknown;
  readonly webConsolePort?: unknown;
  readonly checkIntervalMs?: unknown;
  readonly restartWaitSeconds?: unknown;
  readonly injectDelayMs?: unknown;
  readonly scriptGapMs?: unknown;
  readonly autoBroadcastDefault?: unknown;
  readonly serverExecutable?: unknown;
  readonly shippingExecutable?: unknown;
  readonly shippingProcessName?: unknown;
}
