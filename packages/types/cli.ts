export type MapId =
  | "Departure_Persistent"
  | "Expanse_Persistent"
  | "Approach_Persistent";

export type MapChoiceKey = "1" | "2" | "3";

export type PostMatchAction = "reconfigure" | "restart" | "abort";

export interface MapOption {
  readonly id: MapId;
  readonly label: string;
}

export interface ServerSettings {
  readonly maxPlayers: number;
  readonly daysBeforeBlizzard: number;
  readonly dayMinutes: number;
  readonly predatorDamage: number;
  readonly coldIntensity: number;
  readonly hungerRate: number;
  readonly coalBurnRate: number;
  readonly thralls: number;
}

export interface NumericRangePromptOptions {
  readonly label: string;
  readonly min: number;
  readonly max: number;
  readonly defaultValue: number;
  readonly integerOnly: boolean;
}

export interface WebConsoleSetupConfig {
  readonly webConsolePort: number;
  readonly consolePatchPort: number;
  readonly autoBroadcast: boolean;
}

export interface InfraConfig extends WebConsoleSetupConfig {
  readonly serverPorts: readonly [number, number];
}

export interface MatchConfig {
  readonly mapId: MapId;
  readonly mapLabel: string;
  readonly settings: ServerSettings;
  readonly selectedOptionNumbers: readonly number[];
  readonly selectedOptionLabels: readonly string[];
  readonly ueMapArgument: string;
}

export interface LaunchPlan extends InfraConfig, MatchConfig {}
