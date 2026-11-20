import type { LaunchPlan } from "./cli";
import type { AppConfig } from "./config";

export interface RunSupervisorOptions {
  readonly plan: LaunchPlan;
  readonly config: AppConfig;
  readonly signal: AbortSignal;
}
