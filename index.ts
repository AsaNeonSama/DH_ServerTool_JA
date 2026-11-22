import { existsSync } from "fs";
import { dirname, join } from "path";
import { runServerSetupCli, runWebConsoleConfigCli } from "./packages/cli";
import {
  APP_CONSOLE_TITLE,
  C,
  DEFAULT_CONFIG,
  DEFAULT_PATCH_SCRIPTS,
  DEFAULT_PUBLIC_IP_TIMEOUT_MS,
  LEGACY_GADGET_DLL_FILENAME,
} from "./packages/constants";
import {
  ensureServerWorkingDir,
  fetchPublicIp,
  formatSeconds,
  loadOrCreateConfig,
} from "./packages/core";
import { APP_MESSAGES } from "./packages/messages";
import { runServerSupervisor } from "./packages/server";
import {
  enterTabbedMode,
  exitTabbedMode,
  isInteractiveTty,
  println,
  setupTerminal,
  waitForExitKey,
} from "./packages/terminal";
import type {
  AppConfig,
  LaunchPlan,
  PatchScript,
  PublicIpResult,
  WebConsoleInstance,
  WebConsoleSetupConfig,
} from "./packages/types";
import { startWebConsole } from "./packages/webConsole";
import { closeLauncherJob } from "./packages/win32";

async function main(): Promise<void> {
  setupTerminal(APP_CONSOLE_TITLE);

  // Author & Antigravity branding banner
  println(
    `${C.LIGHT_CYAN} * ${APP_MESSAGES.authorLabel} ${C.LIGHT_WHITE}${APP_MESSAGES.authorName}  ${C.LIGHT_YELLOW}${APP_MESSAGES.brandBuild} ${C.LIGHT_GREEN}${APP_MESSAGES.brandWith} ${C.LIGHT_BLUE}${APP_MESSAGES.brandAntigravity}`
  );

  // Ensure working directory contains the server binary (relative paths only)
  if (!ensureServerWorkingDir(DEFAULT_CONFIG.shippingExecutable)) {
    println(`${C.RED} * ${APP_MESSAGES.missingServerDirError}`);
    await waitForExitKey();
    process.exit(1);
  }

  // Fetch public IP concurrently with loading config.json using Promise.allSettled
  const [ipSettled, configSettled]: [
    PromiseSettledResult<PublicIpResult>,
    PromiseSettledResult<AppConfig>
  ] = await Promise.allSettled([
    fetchPublicIp(DEFAULT_PUBLIC_IP_TIMEOUT_MS),
    loadOrCreateConfig(),
  ]);

  const ipResult: PublicIpResult =
    ipSettled.status === "fulfilled"
      ? ipSettled.value
      : { ip: null, error: String(ipSettled.reason) };

  const config: AppConfig =
    configSettled.status === "fulfilled"
      ? configSettled.value
      : { ...DEFAULT_CONFIG };

  if (ipResult.ip) {
    println(
      `${C.LIGHT_CYAN} * ${APP_MESSAGES.publicIpLabel} ${C.LIGHT_GREEN}${ipResult.ip}`
    );
  } else {
    println(
      `${C.RED} * ${APP_MESSAGES.publicIpFetchFailed(ipResult.error ?? "unknown error")}`
    );
  }

  if (!existsSync(config.shippingExecutable)) {
    println(
      `${C.RED} * ${APP_MESSAGES.missingShippingExecutable(config.shippingExecutable)}`
    );
    println(`${C.RED} * ${APP_MESSAGES.missingServerDirError}`);
    await waitForExitKey();
    process.exit(1);
  }

  const legacyFridaDll: string = join(
    dirname(config.shippingExecutable),
    LEGACY_GADGET_DLL_FILENAME
  );
  if (existsSync(legacyFridaDll)) {
    println(
      `${C.LIGHT_YELLOW} * ${APP_MESSAGES.legacyFridaDllDetected(LEGACY_GADGET_DLL_FILENAME, legacyFridaDll)}`
    );
    println(
      `${C.LIGHT_YELLOW} * ${APP_MESSAGES.legacyFridaDllAdvice(config.shippingProcessName)}`
    );
  }

  println(
    `${C.LIGHT_BLACK} * ${APP_MESSAGES.configSummary({
      serverPort1: config.serverPorts[0],
      serverPort2: config.serverPorts[1],
      webConsolePort: config.webConsolePort,
      consolePatchPort: config.consolePatchPort,
      checkIntervalText: formatSeconds(config.checkIntervalMs),
      restartWaitSeconds: config.restartWaitSeconds,
    })}`
  );
  println();

  // Step 1: Web Console TCP port & Auto-broadcast settings (first section)
  const webConfig: WebConsoleSetupConfig = await runWebConsoleConfigCli(config);

  // Step 2: Start integrated Web Console IMMEDIATELY and display its access URLs/token
  let webConsole: WebConsoleInstance | null = null;
  try {
    webConsole = startWebConsole({
      webPort: webConfig.webConsolePort,
      consolePatchPort: webConfig.consolePatchPort,
      autoBroadcast: webConfig.autoBroadcast,
      publicIp: ipResult.ip,
    });
  } catch (err: unknown) {
    const msg: string = err instanceof Error ? err.message : String(err);
    println(
      `${C.RED} * ${APP_MESSAGES.webConsoleStartFailed(webConfig.webConsolePort, msg)}`
    );
    await waitForExitKey();
    process.exit(1);
  }

  // Register graceful shutdown as soon as Web Console is live
  const stopController: AbortController = new AbortController();
  const handleShutdown = (): void => {
    if (stopController.signal.aborted) return;
    exitTabbedMode();
    println(`${C.LIGHT_YELLOW} * ${APP_MESSAGES.shutdownSignalReceived}`);
    stopController.abort();
    webConsole?.stop();
  };

  process.on("SIGINT", handleShutdown);
  process.on("SIGTERM", handleShutdown);

  if (isInteractiveTty()) {
    enterTabbedMode(webConsole.displayInfo, "server");
  }

  try {
    // Step 3: Server ports, map, match settings, and MOD selection (second section)
    const plan: LaunchPlan | null = await runServerSetupCli(
      config,
      webConfig,
      stopController.signal
    );
    if (!plan || stopController.signal.aborted) {
      return;
    }

    const defaultModNames: string = DEFAULT_PATCH_SCRIPTS.map(
      (patch: PatchScript): string => patch.label
    ).join(", ");
    const selectedModNames: string =
      plan.selectedOptionLabels.length > 0
        ? plan.selectedOptionLabels.join(", ")
        : APP_MESSAGES.noneLabel;

    // Display launch summary (separate visual category)
    println();
    println(`${C.LIGHT_CYAN} ${APP_MESSAGES.summaryHeader}`);
    println(
      `${C.LIGHT_WHITE} * ${APP_MESSAGES.summaryMapLabel} ${C.LIGHT_GREEN}${plan.mapLabel} (${plan.mapId})`
    );
    println(
      `${C.LIGHT_WHITE} * ${APP_MESSAGES.summaryServerPortLabel} ${C.LIGHT_GREEN}${plan.serverPorts[0]} <-> ${plan.serverPorts[1]}`
    );
    println(
      `${C.LIGHT_WHITE} * ${APP_MESSAGES.summaryWebConsoleLabel} ${C.LIGHT_GREEN}${plan.webConsolePort} ${C.LIGHT_BLACK}${APP_MESSAGES.summaryInternalPatchPort(plan.consolePatchPort)}`
    );
    println(
      `${C.LIGHT_WHITE} * ${APP_MESSAGES.summaryDefaultModsLabel} ${C.CYAN}${defaultModNames}`
    );
    println(
      `${C.LIGHT_WHITE} * ${APP_MESSAGES.summarySelectedModsLabel} ${C.CYAN}${selectedModNames}`
    );
    println(`${C.LIGHT_CYAN} ${APP_MESSAGES.summaryFooter}`);

    await runServerSupervisor({
      plan,
      config,
      signal: stopController.signal,
    });
  } finally {
    process.off("SIGINT", handleShutdown);
    process.off("SIGTERM", handleShutdown);
    exitTabbedMode();
    webConsole.stop();
    closeLauncherJob();
    println(`${C.LIGHT_GREEN} * ${APP_MESSAGES.exitedCleanly}`);
  }
}

let fatalHandling: boolean = false;

function handleFatalError(err: unknown): void {
  if (fatalHandling) return;
  fatalHandling = true;
  exitTabbedMode();
  closeLauncherJob();
  const msg: string =
    err instanceof Error ? (err.stack ?? err.message) : String(err);
  println(`${C.RED} * ${APP_MESSAGES.unexpectedErrorHeader}\n${msg}`);
  process.exit(1);
}

process.on("uncaughtException", handleFatalError);
process.on("unhandledRejection", handleFatalError);

main().catch(async (err: unknown): Promise<void> => {
  exitTabbedMode();
  closeLauncherJob();
  if (err instanceof Error && err.name === "AbortError") {
    println(`${C.LIGHT_GREEN} * ${APP_MESSAGES.exitedCleanly}`);
    process.exit(0);
  }
  const msg: string =
    err instanceof Error ? (err.stack ?? err.message) : String(err);
  println(`${C.RED} * ${APP_MESSAGES.unexpectedErrorHeader}\n${msg}`);
  await waitForExitKey();
  process.exit(1);
});
