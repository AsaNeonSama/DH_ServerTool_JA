import { C, DEFAULT_SETTINGS, MAP_OPTIONS } from "../constants";
import { CLI_MESSAGES } from "../messages";
import { println } from "../terminal";
import type { Prompter } from "../terminal";
import type { MapId, MapOption, MatchConfig, ServerSettings } from "../types";
import { promptModSelection } from "./modSelector";
import { askNumberInRange } from "./prompts";

export function buildUeMapArgument(
  mapId: MapId,
  settings: ServerSettings
): string {
  return (
    `${mapId}` +
    `?maxplayers=${settings.maxPlayers}` +
    `?daysbeforeblizzard=${settings.daysBeforeBlizzard}` +
    `?dayminutes=${settings.dayMinutes}` +
    `?predatordamage=${settings.predatorDamage}` +
    `?coldintensity=${settings.coldIntensity}` +
    `?hungerrate=${settings.hungerRate}` +
    `?coalburnrate=${settings.coalBurnRate}` +
    `?thralls=${settings.thralls}`
  );
}

export async function promptMatchConfig(
  prompter: Prompter,
  baseSettings: ServerSettings,
  signal?: AbortSignal
): Promise<MatchConfig> {
  // 1. Map selection
  let mapChoice: MapOption;
  while (true) {
    const rawMap: string = await prompter.ask(
      `${C.LIGHT_YELLOW} * ${CLI_MESSAGES.mapPromptPrefix}(${C.LIGHT_GREEN}${CLI_MESSAGES.mapPromptChoices}${C.LIGHT_YELLOW}): `,
      signal
    );
    if (rawMap === "1" || rawMap === "2" || rawMap === "3") {
      mapChoice = MAP_OPTIONS[rawMap];
      break;
    }
    println(`${C.RED} * ${CLI_MESSAGES.invalidChoiceRetry}`);
  }

  // 2. Server configuration (default or edit)
  println();
  let settings: ServerSettings = { ...DEFAULT_SETTINGS };
  while (true) {
    const rawMode: string = await prompter.ask(
      `${C.LIGHT_YELLOW} * ${CLI_MESSAGES.serverConfigPromptPrefix}(${C.LIGHT_GREEN}${CLI_MESSAGES.serverConfigChoices}${C.LIGHT_YELLOW}): `,
      signal
    );
    if (rawMode === "1") {
      break;
    }
    if (rawMode === "2") {
      println();
      settings = {
        maxPlayers: await askNumberInRange(
          prompter,
          {
            label: CLI_MESSAGES.settingLabels.maxPlayers,
            min: 1,
            max: 8,
            defaultValue: baseSettings.maxPlayers,
            integerOnly: true,
          },
          signal
        ),
        daysBeforeBlizzard: await askNumberInRange(
          prompter,
          {
            label: CLI_MESSAGES.settingLabels.daysBeforeBlizzard,
            min: 2,
            max: 7,
            defaultValue: baseSettings.daysBeforeBlizzard,
            integerOnly: true,
          },
          signal
        ),
        dayMinutes: await askNumberInRange(
          prompter,
          {
            label: CLI_MESSAGES.settingLabels.dayMinutes,
            min: 5,
            max: 16,
            defaultValue: baseSettings.dayMinutes,
            integerOnly: true,
          },
          signal
        ),
        predatorDamage: await askNumberInRange(
          prompter,
          {
            label: CLI_MESSAGES.settingLabels.predatorDamage,
            min: 0.25,
            max: 3,
            defaultValue: baseSettings.predatorDamage,
            integerOnly: false,
          },
          signal
        ),
        coldIntensity: await askNumberInRange(
          prompter,
          {
            label: CLI_MESSAGES.settingLabels.coldIntensity,
            min: 0.25,
            max: 3,
            defaultValue: baseSettings.coldIntensity,
            integerOnly: false,
          },
          signal
        ),
        hungerRate: await askNumberInRange(
          prompter,
          {
            label: CLI_MESSAGES.settingLabels.hungerRate,
            min: 0.25,
            max: 3,
            defaultValue: baseSettings.hungerRate,
            integerOnly: false,
          },
          signal
        ),
        coalBurnRate: await askNumberInRange(
          prompter,
          {
            label: CLI_MESSAGES.settingLabels.coalBurnRate,
            min: 0.1,
            max: 2,
            defaultValue: baseSettings.coalBurnRate,
            integerOnly: false,
          },
          signal
        ),
        thralls: await askNumberInRange(
          prompter,
          {
            label: CLI_MESSAGES.settingLabels.thralls,
            min: 0,
            max: 8,
            defaultValue: baseSettings.thralls,
            integerOnly: true,
          },
          signal
        ),
      };
      break;
    }
    println(`${C.RED} * ${CLI_MESSAGES.invalidChoiceRetry}`);
  }

  // 3. Mod selection loop
  println();
  const {
    selectedOptionNumbers,
    selectedOptionLabels,
  }: {
    readonly selectedOptionNumbers: number[];
    readonly selectedOptionLabels: string[];
  } = await promptModSelection(prompter, signal);

  const ueMapArgument: string = buildUeMapArgument(mapChoice.id, settings);

  return {
    mapId: mapChoice.id,
    mapLabel: mapChoice.label,
    settings,
    selectedOptionNumbers,
    selectedOptionLabels,
    ueMapArgument,
  };
}
