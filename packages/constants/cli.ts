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
  "1": { id: "Departure_Persistent", label: "山頂" },
  "2": { id: "Expanse_Persistent", label: "広漠" },
  "3": { id: "Approach_Persistent", label: "進入路" },
};
