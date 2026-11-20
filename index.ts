import { existsSync } from "fs";
import { dirname, join } from "path";
import { runServerSetupCli, runWebConsoleConfigCli } from "./packages/cli";
import { ensureServerWorkingDir, loadOrCreateConfig } from "./packages/config";
import {
  APP_CONSOLE_TITLE,
  C,
  DEFAULT_CONFIG,
  DEFAULT_PATCH_SCRIPTS,
  DEFAULT_PUBLIC_IP_TIMEOUT_MS,
  LEGACY_GADGET_DLL_FILENAME,
} from "./packages/constants";
import { fetchPublicIp } from "./packages/network";
import { runServerSupervisor } from "./packages/serverManager";
import {
  enterTabbedMode,
  exitTabbedMode,
  formatSeconds,
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

async function main(): Promise<void> {
  setupTerminal(APP_CONSOLE_TITLE);

  // Author & Antigravity branding banner
  println(
    `${C.LIGHT_CYAN} * ソフト作者: ${C.LIGHT_WHITE}アサネオン  ${C.LIGHT_YELLOW}Build ${C.LIGHT_GREEN}With ${C.LIGHT_BLUE}Antigravity`
  );

  // Ensure working directory contains the server binary (relative paths only)
  if (!ensureServerWorkingDir(DEFAULT_CONFIG.shippingExecutable)) {
    println(
      `${C.RED} * 本ソフトをサーバーのフォルダの下に入れた後で再起動してください!`
    );
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
    println(`${C.LIGHT_CYAN} * パブリックIP: ${C.LIGHT_GREEN}${ipResult.ip}`);
  } else {
    println(
      `${C.RED} * パブリックIPの取得に失敗しました: ${ipResult.error ?? "unknown error"}`
    );
  }

  if (!existsSync(config.shippingExecutable)) {
    println(
      `${C.RED} * サーバー実行ファイルが見つかりません: ${config.shippingExecutable}`
    );
    println(
      `${C.RED} * 本ソフトをサーバーのフォルダの下に入れた後で再起動してください!`
    );
    await waitForExitKey();
    process.exit(1);
  }

  const legacyFridaDll: string = join(
    dirname(config.shippingExecutable),
    LEGACY_GADGET_DLL_FILENAME
  );
  if (existsSync(legacyFridaDll)) {
    println(
      `${C.LIGHT_YELLOW} * 警告: 旧パッチ用の ${LEGACY_GADGET_DLL_FILENAME} (${legacyFridaDll}) が検出されました。`
    );
    println(
      `${C.LIGHT_YELLOW} * 原生サーバーで安定動作させるため、未改造の ${config.shippingProcessName} を使用し frida.dll / frida.config / Patches を削除してください。`
    );
  }

  println(
    `${C.LIGHT_BLACK} * config.json: サーバーポート=${config.serverPorts[0]}/${config.serverPorts[1]}, Webコンソール=${config.webConsolePort}, パッチ連携=${config.consolePatchPort}, 監視間隔=${formatSeconds(config.checkIntervalMs)}, 再設定待機=${config.restartWaitSeconds}s, 注入方式=Frida直接起動注入(spawn)`
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
      `${C.RED} * Webコンソールの起動に失敗しました (ポート ${webConfig.webConsolePort}): ${msg}`
    );
    await waitForExitKey();
    process.exit(1);
  }

  // Register graceful shutdown as soon as Web Console is live
  const stopController: AbortController = new AbortController();
  const handleShutdown = (): void => {
    if (stopController.signal.aborted) return;
    exitTabbedMode();
    println(
      `${C.LIGHT_YELLOW} * 終了シグナルを受信しました。サーバーとWebコンソールを停止しています...`
    );
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
        : "なし";

    // Display launch summary (separate visual category)
    println();
    println(
      `${C.LIGHT_CYAN} ======================= 起動設定まとめ =======================`
    );
    println(
      `${C.LIGHT_WHITE} * マップ:         ${C.LIGHT_GREEN}${plan.mapLabel} (${plan.mapId})`
    );
    println(
      `${C.LIGHT_WHITE} * サーバーポート: ${C.LIGHT_GREEN}${plan.serverPorts[0]} <-> ${plan.serverPorts[1]}`
    );
    println(
      `${C.LIGHT_WHITE} * Webコンソール:  ${C.LIGHT_GREEN}${plan.webConsolePort} ${C.LIGHT_BLACK}(内部パッチポート: ${plan.consolePatchPort})`
    );
    println(`${C.LIGHT_WHITE} * 自動注入MOD:    ${C.CYAN}${defaultModNames}`);
    println(`${C.LIGHT_WHITE} * 選択済みMOD:    ${C.CYAN}${selectedModNames}`);
    println(
      `${C.LIGHT_CYAN} ==============================================================`
    );

    await runServerSupervisor({
      plan,
      config,
      signal: stopController.signal,
    });
  } finally {
    exitTabbedMode();
    webConsole.stop();
    println(`${C.LIGHT_GREEN} * 正常に終了しました。`);
  }
}

main().catch(async (err: unknown): Promise<void> => {
  exitTabbedMode();
  if (err instanceof Error && err.name === "AbortError") {
    println(`${C.LIGHT_GREEN} * 正常に終了しました。`);
    process.exit(0);
  }
  const msg: string = err instanceof Error ? (err.stack ?? err.message) : String(err);
  println(`${C.RED} * 予期しないエラーが発生しました:\n${msg}`);
  await waitForExitKey();
  process.exit(1);
});
