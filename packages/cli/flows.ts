import {
  C,
  DEFAULT_SETTINGS,
  MAX_VALID_PORT,
  MIN_VALID_PORT,
} from "../constants";
import { CLI_MESSAGES } from "../messages";
import { println, Prompter, resetServerTabRound } from "../terminal";
import type {
  AppConfig,
  LaunchPlan,
  MatchConfig,
  WebConsoleSetupConfig,
} from "../types";
import { promptMatchConfig } from "./matchSetup";
import { isAbortError, parsePortInput } from "./prompts";

/**
 * Section 1: Web Console TCP port & Auto-Broadcast settings.
 * Executed first so the Web Console starts and outputs its URLs immediately.
 */
export async function runWebConsoleConfigCli(
  config: AppConfig
): Promise<WebConsoleSetupConfig> {
  const prompter: Prompter = new Prompter();
  try {
    println(`${C.LIGHT_CYAN} ${CLI_MESSAGES.webConsoleSectionHeader}`);

    let webConsolePort: number = config.webConsolePort;
    while (true) {
      const rawWeb: string = await prompter.ask(
        `${C.LIGHT_WHITE} * ${CLI_MESSAGES.webPortPrompt} ${C.LIGHT_BLACK}[Enter=${config.webConsolePort}]${C.LIGHT_WHITE}: `
      );
      const parsedWeb: number | null = parsePortInput(
        rawWeb,
        config.webConsolePort
      );
      if (parsedWeb === null) {
        println(
          `${C.RED} * ${CLI_MESSAGES.portRangeError(MIN_VALID_PORT, MAX_VALID_PORT)}`
        );
        continue;
      }
      if (parsedWeb === config.consolePatchPort) {
        println(
          `${C.RED} * ${CLI_MESSAGES.webPortConflictError(config.consolePatchPort)}`
        );
        continue;
      }
      webConsolePort = parsedWeb;
      break;
    }

    let autoBroadcast: boolean = config.autoBroadcastDefault;
    const defaultBroadcastNum: number = config.autoBroadcastDefault ? 1 : 0;
    while (true) {
      const rawBroadcast: string = await prompter.ask(
        `${C.LIGHT_WHITE} * ${CLI_MESSAGES.autoBroadcastPromptPrefix}(${C.LIGHT_GREEN}${CLI_MESSAGES.autoBroadcastChoices}${C.LIGHT_WHITE}) ${C.LIGHT_BLACK}[Enter=${defaultBroadcastNum}]${C.LIGHT_WHITE}: `
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
      println(`${C.RED} * ${CLI_MESSAGES.invalidChoiceRetry}`);
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
    println(`${C.LIGHT_CYAN} ${CLI_MESSAGES.serverSetupSectionHeader}`);

    let port1: number = config.serverPorts[0];
    let port2: number = config.serverPorts[1];
    while (true) {
      const raw1: string = await prompter.ask(
        `${C.LIGHT_WHITE} * ${CLI_MESSAGES.serverPortPromptPrefix}(${C.LIGHT_GREEN}1/2${C.LIGHT_WHITE}) ${C.LIGHT_BLACK}[Enter=${config.serverPorts[0]}]${C.LIGHT_WHITE}: `,
        signal
      );
      const raw2: string = await prompter.ask(
        `${C.LIGHT_WHITE} * ${CLI_MESSAGES.serverPortPromptPrefix}(${C.LIGHT_GREEN}2/2${C.LIGHT_WHITE}) ${C.LIGHT_BLACK}[Enter=${config.serverPorts[1]}]${C.LIGHT_WHITE}: `,
        signal
      );

      const p1: number | null = parsePortInput(raw1, config.serverPorts[0]);
      const p2: number | null = parsePortInput(raw2, config.serverPorts[1]);
      if (p1 === null || p2 === null) {
        println(
          `${C.RED} * ${CLI_MESSAGES.portRangeError(MIN_VALID_PORT, MAX_VALID_PORT)}`
        );
        continue;
      }
      if (p1 === p2) {
        println(`${C.RED} * ${CLI_MESSAGES.serverPortDuplicateError}`);
        continue;
      }
      if (
        p1 === webConfig.webConsolePort ||
        p2 === webConfig.webConsolePort ||
        p1 === webConfig.consolePatchPort ||
        p2 === webConfig.consolePatchPort
      ) {
        println(
          `${C.RED} * ${CLI_MESSAGES.serverPortWebConflictError(webConfig.webConsolePort, webConfig.consolePatchPort)}`
        );
        continue;
      }
      port1 = p1;
      port2 = p2;
      break;
    }

    // Rotate clean before match/MOD configuration so Tab 2 retains only this round's config & injection logs
    resetServerTabRound();
    println(`${C.LIGHT_CYAN} ${CLI_MESSAGES.serverSetupSectionHeader}`);
    println(
      `${C.LIGHT_WHITE} * ${CLI_MESSAGES.serverPortsSelectedLabel} ${C.LIGHT_GREEN}${port1} <-> ${port2}`
    );

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
    println(`${C.LIGHT_CYAN} ${CLI_MESSAGES.reconfigureSectionHeader}`);
    println(
      `${C.LIGHT_BLACK} * ${CLI_MESSAGES.reconfigurePreservedNotice(currentPlan.serverPorts[0], currentPlan.serverPorts[1])}`
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
    println(`${C.RED} * ${CLI_MESSAGES.reconfigureInterruptedNotice(msg)}`);
    return currentPlan;
  } finally {
    prompter.close();
  }
}
