import type { AppConfig } from "../types";

export const CONFIG_RELATIVE_PATH: string = "./config.json";

export const MIN_VALID_PORT: number = 1024;
export const MAX_VALID_PORT: number = 65535;
export const MIN_CHECK_INTERVAL_MS: number = 200;
export const MIN_RESTART_WAIT_SECONDS: number = 1;

export const DEFAULT_CONFIG: AppConfig = {
  serverPorts: [7895, 7897],
  consolePatchPort: 7893,
  webConsolePort: 7891,
  checkIntervalMs: 1000,
  restartWaitSeconds: 10,
  autoBroadcastDefault: true,
  serverExecutable: "./DreadHungerServer.exe",
  shippingExecutable:
    "./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe",
  shippingProcessName: "DreadHungerServer-Win64-Shipping.exe",
};
