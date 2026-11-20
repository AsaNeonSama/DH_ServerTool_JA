import {
  C,
  DEFAULT_PATCH_SCRIPTS,
  DEFAULT_SETTINGS,
  MAP_OPTIONS,
  MAX_VALID_PORT,
  MIN_VALID_PORT,
  MOD_MENU_COL_WIDTH,
  MOD_MENU_COLS,
  SELECTABLE_MOD_OPTIONS,
} from "./constants";
import { padDisplayWidth, println, Prompter } from "./terminal";
import type {
  AppConfig,
  LaunchPlan,
  MapId,
  MapOption,
  MatchConfig,
  ModMenuOption,
  NumericRangePromptOptions,
  PatchScript,
  ServerSettings,
  WebConsoleSetupConfig,
} from "./types";

function parsePortInput(raw: string, fallback: number): number | null {
  const value: number = raw === "" ? fallback : Number(raw);
  if (!Number.isInteger(value) || value < MIN_VALID_PORT || value > MAX_VALID_PORT) {
    return null;
  }
  return value;
}

async function askNumberInRange(
  prompter: Prompter,
  options: NumericRangePromptOptions,
  signal?: AbortSignal
): Promise<number> {
  const { label, min, max, defaultValue, integerOnly }: NumericRangePromptOptions = options;
  while (true) {
    const raw: string = await prompter.ask(
      `${C.LIGHT_CYAN} * ${label}(${min}-${max}) ${C.LIGHT_BLACK}[Enter=${defaultValue}]${C.LIGHT_CYAN}: `,
      signal
    );
    if (raw === "") {
      return defaultValue;
    }
    const parsed: number = Number(raw);
    const validType: boolean = integerOnly
      ? Number.isInteger(parsed)
      : Number.isFinite(parsed);
    if (!validType || parsed < min || parsed > max) {
      println();
      println(
        `${C.RED} * 規格外の選択肢を入力しました、やり直してください!`
      );
      println();
      continue;
    }
    return parsed;
  }
}

function renderModMenuTable(): void {
  const rows: number = Math.ceil(SELECTABLE_MOD_OPTIONS.length / MOD_MENU_COLS);

  for (let r: number = 0; r < rows; r++) {
    let line: string = " ";
    for (let c: number = 0; c < MOD_MENU_COLS; c++) {
      const idx: number = c * rows + r;
      const opt: ModMenuOption | undefined = SELECTABLE_MOD_OPTIONS[idx];
      if (!opt) continue;
      const suffix: string = opt.kind === "mode" ? " [セット]" : "";
      const cell: string = `${opt.number}:${opt.label}${suffix}`;
      line += c < MOD_MENU_COLS - 1 ? padDisplayWidth(cell, MOD_MENU_COL_WIDTH) : cell;
    }
    println(`${C.CYAN}${line}`);
  }
}

function formatSelectedLabels(selectedNumbers: readonly number[]): string {
  return selectedNumbers
    .map((n: number): string => {
      const found: ModMenuOption | undefined = SELECTABLE_MOD_OPTIONS.find(
        (item: ModMenuOption): boolean => item.number === n
      );
      return found?.label ?? String(n);
    })
    .join(", ");
}

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

async function promptMatchConfig(
  prompter: Prompter,
  baseSettings: ServerSettings,
  signal?: AbortSignal
): Promise<MatchConfig> {
  // 1. Map selection
  let mapChoice: MapOption;
  println();
  while (true) {
    const rawMap: string = await prompter.ask(
      `${C.LIGHT_YELLOW} * マップを設定してください(${C.LIGHT_GREEN}1:山頂, 2:広漠, 3:進入路${C.LIGHT_YELLOW}): `,
      signal
    );
    if (rawMap === "1" || rawMap === "2" || rawMap === "3") {
      mapChoice = MAP_OPTIONS[rawMap];
      break;
    }
    println();
    println(`${C.RED} * 規格外の選択肢を入力しました、やり直してください!`);
    println();
  }

  // 2. Server configuration (default or edit)
  let settings: ServerSettings = { ...DEFAULT_SETTINGS };
  println();
  while (true) {
    const rawMode: string = await prompter.ask(
      `${C.LIGHT_YELLOW} * サーバーの配置を設定してください(${C.LIGHT_GREEN}1:デフォルト設定, 2:設定を編集する${C.LIGHT_YELLOW}): `,
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
            label: "最大人数設定",
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
            label: "暴風雪前の日数設定",
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
            label: "一日の時間設定",
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
            label: "野生動物ダメージ設定",
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
            label: "寒さ強度設定",
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
            label: "ハングリー強度設定",
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
            label: "石炭燃焼率設定",
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
            label: "傀儡人数設定",
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
    println();
    println(`${C.RED} * 規格外の選択肢を入力しました、やり直してください!`);
    println();
  }

  // 3. Mod selection loop
  println();
  const defaultLabels: string = DEFAULT_PATCH_SCRIPTS.map(
    (patch: PatchScript): string => patch.label
  ).join(", ");
  println(
    `${C.LIGHT_CYAN} * デフォルトMOD(自動注入): ${C.LIGHT_GREEN}${defaultLabels}`
  );
  println(
    `${C.LIGHT_YELLOW} 下記のナンバーで機能を選択してください(選択済みの場合は ${C.LIGHT_GREEN}Y/y ${C.LIGHT_YELLOW}を入力して確認、${C.LIGHT_GREEN}C/c ${C.LIGHT_YELLOW}で選択クリア): `
  );
  println();
  renderModMenuTable();
  println();

  const selectedNumbers: number[] = [];
  while (true) {
    const rawChoice: string = await prompter.ask(
      `${C.LIGHT_CYAN} * 選択肢を入力してください: `,
      signal
    );
    const lower: string = rawChoice.toLowerCase();

    if (lower === "y") {
      break;
    }

    if (lower === "c") {
      selectedNumbers.length = 0;
      println(`${C.LIGHT_YELLOW} * 選択済みのスクリプトをクリアしました。`);
      println();
      continue;
    }

    const num: number = Number(rawChoice);
    const matched: ModMenuOption | undefined = Number.isInteger(num)
      ? SELECTABLE_MOD_OPTIONS.find((item: ModMenuOption): boolean => item.number === num)
      : undefined;

    if (!matched) {
      println();
      println(`${C.RED} * 規格外の選択肢です!`);
      println();
      continue;
    }

    if (selectedNumbers.includes(matched.number)) {
      const currentNames: string = formatSelectedLabels(selectedNumbers);
      println();
      println(
        `${C.RED} * 入力済みの選択肢です。現在選択済みのスクリプトは: `
      );
      println(`${C.CYAN} ${currentNames} `);
      println();
      continue;
    }

    selectedNumbers.push(matched.number);
    const currentNames: string = formatSelectedLabels(selectedNumbers);
    println(
      `${C.LIGHT_GREEN} * 追加しました: ${matched.label} ${C.LIGHT_BLACK}(選択中: ${currentNames})`
    );
  }

  const selectedOptionLabels: string[] = selectedNumbers.map((n: number): string => {
    const found: ModMenuOption | undefined = SELECTABLE_MOD_OPTIONS.find(
      (item: ModMenuOption): boolean => item.number === n
    );
    return found?.label ?? String(n);
  });

  const ueMapArgument: string = buildUeMapArgument(mapChoice.id, settings);

  return {
    mapId: mapChoice.id,
    mapLabel: mapChoice.label,
    settings,
    selectedOptionNumbers: selectedNumbers,
    selectedOptionLabels,
    ueMapArgument,
  };
}

/**
 * Section 1: Web Console TCP port & Auto-Broadcast settings.
 * Executed first so the Web Console starts and outputs its URLs immediately.
 */
export async function runWebConsoleConfigCli(
  config: AppConfig
): Promise<WebConsoleSetupConfig> {
  const prompter: Prompter = new Prompter();
  try {
    println(
      `${C.LIGHT_CYAN} -------------------- [Webコンソール設定] --------------------`
    );

    let webConsolePort: number = config.webConsolePort;
    while (true) {
      const rawWeb: string = await prompter.ask(
        `${C.LIGHT_WHITE} * Webコンソールに使うTCPポートを入れてください ${C.LIGHT_BLACK}[Enter=${config.webConsolePort}]${C.LIGHT_WHITE}: `
      );
      const parsedWeb: number | null = parsePortInput(rawWeb, config.webConsolePort);
      if (parsedWeb === null) {
        println();
        println(
          `${C.RED} * 無効な入力です。${MIN_VALID_PORT}〜${MAX_VALID_PORT}の数値を入力してください!`
        );
        println();
        continue;
      }
      if (parsedWeb === config.consolePatchPort) {
        println();
        println(
          `${C.RED} * コンソールパッチ内部ポート(${config.consolePatchPort})と重複しています。別のポートを指定してください!`
        );
        println();
        continue;
      }
      webConsolePort = parsedWeb;
      break;
    }

    let autoBroadcast: boolean = config.autoBroadcastDefault;
    const defaultBroadcastNum: number = config.autoBroadcastDefault ? 1 : 0;
    while (true) {
      const rawBroadcast: string = await prompter.ask(
        `${C.LIGHT_WHITE} * 自動ブロードキャストスイッチ(${C.LIGHT_GREEN}1:オン, 0:オフ${C.LIGHT_WHITE}) ${C.LIGHT_BLACK}[Enter=${defaultBroadcastNum}]${C.LIGHT_WHITE}: `
      );
      if (rawBroadcast === "") {
        autoBroadcast = config.autoBroadcastDefault;
        break;
      }
      if (rawBroadcast === "1") {
        autoBroadcast = true;
        break;
      }
      if (rawBroadcast === "0") {
        autoBroadcast = false;
        break;
      }
      println();
      println(`${C.RED} * 規格外の選択肢を入力しました、やり直してください!`);
      println();
    }

    return {
      webConsolePort,
      consolePatchPort: config.consolePatchPort,
      autoBroadcast,
    };
  } finally {
    prompter.close();
  }
}

function isAbortError(err: unknown): boolean {
  return err instanceof Error && err.name === "AbortError";
}

/**
 * Section 2: Game server ports, map, match settings, and MOD selection.
 */
export async function runServerSetupCli(
  config: AppConfig,
  webConfig: WebConsoleSetupConfig,
  signal?: AbortSignal
): Promise<LaunchPlan | null> {
  const prompter: Prompter = new Prompter();
  try {
    println();
    println(
      `${C.LIGHT_CYAN} -------------------- [サーバー・試合設定] --------------------`
    );

    let port1: number = config.serverPorts[0];
    let port2: number = config.serverPorts[1];
    while (true) {
      const raw1: string = await prompter.ask(
        `${C.LIGHT_WHITE} * サーバーに使うポートを入れてください(${C.LIGHT_GREEN}1/2${C.LIGHT_WHITE}) ${C.LIGHT_BLACK}[Enter=${config.serverPorts[0]}]${C.LIGHT_WHITE}: `,
        signal
      );
      const raw2: string = await prompter.ask(
        `${C.LIGHT_WHITE} * サーバーに使うポートを入れてください(${C.LIGHT_GREEN}2/2${C.LIGHT_WHITE}) ${C.LIGHT_BLACK}[Enter=${config.serverPorts[1]}]${C.LIGHT_WHITE}: `,
        signal
      );

      const p1: number | null = parsePortInput(raw1, config.serverPorts[0]);
      const p2: number | null = parsePortInput(raw2, config.serverPorts[1]);
      if (p1 === null || p2 === null) {
        println();
        println(
          `${C.RED} * 無効な入力です。${MIN_VALID_PORT}〜${MAX_VALID_PORT}の数値を入力してください!`
        );
        println();
        continue;
      }
      if (p1 === p2) {
        println();
        println(`${C.RED} * 1つ目と2つ目のポートは別の番号を指定してください!`);
        println();
        continue;
      }
      if (
        p1 === webConfig.webConsolePort ||
        p2 === webConfig.webConsolePort ||
        p1 === webConfig.consolePatchPort ||
        p2 === webConfig.consolePatchPort
      ) {
        println();
        println(
          `${C.RED} * Webコンソール関連ポート(${webConfig.webConsolePort}, ${webConfig.consolePatchPort})と重複しています。別のポートを指定してください!`
        );
        println();
        continue;
      }
      port1 = p1;
      port2 = p2;
      break;
    }

    const matchConfig: MatchConfig = await promptMatchConfig(
      prompter,
      DEFAULT_SETTINGS,
      signal
    );
    return {
      ...webConfig,
      serverPorts: [port1, port2],
      ...matchConfig,
    };
  } catch (err: unknown) {
    if (signal?.aborted || isAbortError(err)) {
      return null;
    }
    throw err;
  } finally {
    prompter.close();
  }
}

export async function runReconfigureMatchCli(
  currentPlan: LaunchPlan,
  signal: AbortSignal
): Promise<LaunchPlan | null> {
  const prompter: Prompter = new Prompter();
  try {
    println();
    println(
      `${C.LIGHT_CYAN} ================== マップ・MOD再設定モード ==================`
    );
    println(
      `${C.LIGHT_BLACK} * Webコンソールとサーバーポート設定はそのまま維持されます`
    );

    const matchConfig: MatchConfig = await promptMatchConfig(
      prompter,
      currentPlan.settings,
      signal
    );

    return {
      ...currentPlan,
      ...matchConfig,
    };
  } catch (err: unknown) {
    if (signal.aborted || isAbortError(err)) {
      return null;
    }
    const msg: string = err instanceof Error ? err.message : String(err);
    println(`${C.RED} * 再設定が中断されました (${msg})。前回設定のまま再起動します。`);
    return currentPlan;
  } finally {
    prompter.close();
  }
}
