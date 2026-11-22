import { CLI_MESSAGES } from "../messages";
import type { MapChoiceKey, MapOption, ServerSettings } from "../types";

export const DEFAULT_SETTINGS: ServerSettings = {
  maxPlayers: 8,
  daysBeforeBlizzard: 3,
  dayMinutes: 9,
  predatorDamage: 1,
  coldIntensity: 1,
  hungerRate: 1,
  coalBurnRate: 1,
  thralls: 2,
};

export const MAP_OPTIONS: Readonly<Record<MapChoiceKey, MapOption>> = {
  "1": { id: "Departure_Persistent", label: CLI_MESSAGES.mapLabels["1"] },
  "2": { id: "Expanse_Persistent", label: CLI_MESSAGES.mapLabels["2"] },
  "3": { id: "Approach_Persistent", label: CLI_MESSAGES.mapLabels["3"] },
};
