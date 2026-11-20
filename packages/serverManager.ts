import { runReconfigureMatchCli } from "./cli";
import {
  C,
  DEFAULT_PATCH_SCRIPTS,
  STARTUP_RETRY_DELAY_MS,
  UDP_MISSING_STREAK_LIMIT,
} from "./constants";
import { spawnAndInjectServer } from "./injector";
import { buildInjectionPlan } from "./mods";
import {
  formatSeconds,
  println,
  resetServerTabRound,
  waitForPostMatchDecision,
} from "./terminal";
import type {
  ActiveInjectionSession,
  LaunchPlan,
  PatchScript,
  PostMatchAction,
  RunSupervisorOptions,
  UdpLookupResult,
} from "./types";
import {
  findUdpListenerPid,
  isProcessAlive,
  terminateProcessByPid,
} from "./win32";

async function sleepAbortable(ms: number, signal: AbortSignal): Promise<boolean> {
  if (signal.aborted) return false;
  const waiter: Promise<boolean> = new Promise(
    (resolve: (value: boolean) => void): void => {
      const onAbort = (): void => {
        clearTimeout(timer);
        resolve(false);
      };
      const timer: ReturnType<typeof setTimeout> = setTimeout((): void => {
        signal.removeEventListener("abort", onAbort);
        resolve(true);
      }, ms);
      signal.addEventListener("abort", onAbort, { once: true });
    }
  );
  return waiter;
}

function printRoundConfigSummary(
  plan: LaunchPlan,
  currentPort: number,
  title: string
): void {
  const defaultModNames: string = DEFAULT_PATCH_SCRIPTS.map(
    (patch: PatchScript): string => patch.label
  ).join(", ");
  const selectedModNames: string =
    plan.selectedOptionLabels.length > 0
      ? plan.selectedOptionLabels.join(", ")
      : "なし";

  println(`${C.LIGHT_CYAN} ${title}`);
  println(
    `${C.LIGHT_WHITE} * マップ:         ${C.LIGHT_GREEN}${plan.mapLabel} (${plan.mapId})`
  );
  println(
    `${C.LIGHT_WHITE} * サーバーポート: ${C.LIGHT_GREEN}${plan.serverPorts[0]} <-> ${plan.serverPorts[1]} ${C.LIGHT_BLACK}(今回: ${currentPort})`
  );
  println(`${C.LIGHT_WHITE} * 自動注入MOD:    ${C.CYAN}${defaultModNames}`);
  println(`${C.LIGHT_WHITE} * 選択済みMOD:    ${C.CYAN}${selectedModNames}`);
  println(
    `${C.LIGHT_CYAN} ============================================================`
  );
}

export async function runServerSupervisor(
  options: RunSupervisorOptions
): Promise<void> {
  const { plan, config, signal }: RunSupervisorOptions = options;
  let activePlan: LaunchPlan = plan;
  let patches: PatchScript[] = buildInjectionPlan(
    activePlan.selectedOptionNumbers,
    activePlan.consolePatchPort
  );

  let currentPort: number = activePlan.serverPorts[0];
  let round: number = 0;
  let pendingRetryNotice: string | null = null;

  while (!signal.aborted) {
    round += 1;

    if (pendingRetryNotice !== null) {
      resetServerTabRound();
      println(
        `${C.LIGHT_YELLOW} * 前回起動失敗のためポート切替再試行 (${pendingRetryNotice})`
      );
      printRoundConfigSummary(
        activePlan,
        currentPort,
        "======================= 起動設定まとめ ======================="
      );
      pendingRetryNotice = null;
    }

    println();
    println(
      `${C.LIGHT_CYAN} ================= サーバー起動 (#${round} / ポート: ${currentPort}) =================`
    );
    println(
      `${C.LIGHT_WHITE} * マップ: ${C.LIGHT_GREEN}${activePlan.mapLabel} (${activePlan.mapId})  ${C.LIGHT_WHITE}ポート: ${C.LIGHT_GREEN}${currentPort}`
    );

    let activeSession: ActiveInjectionSession | null = null;
    let shippingPid: number | null = null;

    try {
      try {
        activeSession = await spawnAndInjectServer({
          shippingExecutable: config.shippingExecutable,
          ueMapArgument: activePlan.ueMapArgument,
          port: currentPort,
          patches,
          scriptGapMs: config.scriptGapMs,
          onDetached: (reason: string): void => {
            if (!signal.aborted) {
              println(
                `${C.LIGHT_YELLOW} * Fridaセッション切断を検知しました (${reason})`
              );
            }
          },
        });
        shippingPid = activeSession.pid;
      } catch (err: unknown) {
        if (signal.aborted) {
          break;
        }
        const msg: string = err instanceof Error ? err.message : String(err);
        println(
          `${C.RED} * サーバー起動・注入エラー: ${msg}`
        );
        println(
          `${C.RED} * ${formatSeconds(STARTUP_RETRY_DELAY_MS)}後にポートを切り替えて再試行します...`
        );
        pendingRetryNotice = msg;
        currentPort =
          currentPort === activePlan.serverPorts[0]
            ? activePlan.serverPorts[1]
            : activePlan.serverPorts[0];
        await sleepAbortable(STARTUP_RETRY_DELAY_MS, signal);
        continue;
      }

      if (signal.aborted) {
        break;
      }

      println(
        `${C.LIGHT_CYAN} * サーバー監視中 (${formatSeconds(config.checkIntervalMs)}間隔 / 終了時に自動ポート切替・再起動します)...`
      );
      println(
        `${C.LIGHT_BLACK} * 終了する場合は Ctrl+C を押してください`
      );

      // Background watchdog loop (tracks UDP port bind lazily so early startup never triggers false restarts)
      let udpEverObserved: boolean = false;
      let udpMissingStreak: number = 0;
      while (!signal.aborted) {
        if (!(await sleepAbortable(config.checkIntervalMs, signal))) {
          break;
        }

        const alive: boolean = isProcessAlive(shippingPid);
        const detached: boolean = activeSession.isDetached();

        if (!alive || detached) {
          break;
        }

        const udp: UdpLookupResult = findUdpListenerPid(currentPort, shippingPid);
        if (udp.available) {
          if (udp.pid === shippingPid) {
            if (!udpEverObserved) {
              udpEverObserved = true;
              println(
                `${C.LIGHT_GREEN} * サーバー起動完了確認 (PID: ${shippingPid}, UDP:${currentPort} リッスン中)`
              );
            }
            udpMissingStreak = 0;
          } else if (udpEverObserved) {
            udpMissingStreak += 1;
            if (udpMissingStreak >= UDP_MISSING_STREAK_LIMIT) {
              println(
                `${C.LIGHT_YELLOW} * ポート ${currentPort} のリッスン停止を検知しました。サーバーを再起動します...`
              );
              terminateProcessByPid(shippingPid);
              break;
            }
          }
        }
      }
    } finally {
      if (activeSession) {
        await activeSession.cleanup();
      }
      if (signal.aborted && shippingPid !== null && isProcessAlive(shippingPid)) {
        terminateProcessByPid(shippingPid);
      }
    }

    if (signal.aborted) {
      break;
    }

    const prevPort: number = currentPort;
    currentPort =
      currentPort === activePlan.serverPorts[0]
        ? activePlan.serverPorts[1]
        : activePlan.serverPorts[0];

    println();
    println(
      `${C.LIGHT_YELLOW} * サーバーの終了を検知しました! 次回起動ポート: ${C.LIGHT_WHITE}${prevPort} -> ${C.LIGHT_GREEN}${currentPort}`
    );

    // Wait restartWaitSeconds (default 10s) so user can press R to change map/settings/mods
    const action: PostMatchAction = await waitForPostMatchDecision(
      config.restartWaitSeconds,
      signal
    );

    if (action === "abort" || signal.aborted) {
      break;
    }

    if (action === "reconfigure") {
      resetServerTabRound();
      const updatedPlan: LaunchPlan | null = await runReconfigureMatchCli(
        activePlan,
        signal
      );
      if (!updatedPlan || signal.aborted) {
        break;
      }
      activePlan = updatedPlan;
      patches = buildInjectionPlan(
        activePlan.selectedOptionNumbers,
        activePlan.consolePatchPort
      );

      println();
      printRoundConfigSummary(
        activePlan,
        currentPort,
        "======================= 更新後の設定 ======================="
      );
    } else {
      resetServerTabRound();
      printRoundConfigSummary(
        activePlan,
        currentPort,
        "================= 起動設定 (前回設定を継続) ================="
      );
    }
  }
}
