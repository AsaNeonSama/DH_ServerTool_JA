import { runReconfigureMatchCli } from "../cli";
import {
  C,
  DEFAULT_PATCH_SCRIPTS,
  DETACH_EXIT_GRACE_MS,
  STARTUP_RETRY_DELAY_MS,
  UDP_MISSING_STREAK_LIMIT,
} from "../constants";
import { formatSeconds } from "../core";
import { APP_MESSAGES, SERVER_MESSAGES } from "../messages";
import {
  println,
  resetServerTabRound,
  waitForPostMatchDecision,
} from "../terminal";
import type {
  ActiveInjectionSession,
  LaunchPlan,
  PatchScript,
  PostMatchAction,
  RunSupervisorOptions,
  UdpLookupResult,
} from "../types";
import {
  findUdpListenerPid,
  isProcessAlive,
  terminateProcessByPid,
} from "../win32";
import { spawnAndInjectServer } from "./injector";
import { buildInjectionPlan } from "./mods";

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
      : APP_MESSAGES.noneLabel;

  println(`${C.LIGHT_CYAN} ${title}`);
  println(
    `${C.LIGHT_WHITE} * ${APP_MESSAGES.summaryMapLabel} ${C.LIGHT_GREEN}${plan.mapLabel} (${plan.mapId})`
  );
  println(
    `${C.LIGHT_WHITE} * ${APP_MESSAGES.summaryServerPortLabel} ${C.LIGHT_GREEN}${plan.serverPorts[0]} <-> ${plan.serverPorts[1]} ${C.LIGHT_BLACK}${SERVER_MESSAGES.roundSummaryCurrentPort(currentPort)}`
  );
  println(
    `${C.LIGHT_WHITE} * ${APP_MESSAGES.summaryDefaultModsLabel} ${C.CYAN}${defaultModNames}`
  );
  println(
    `${C.LIGHT_WHITE} * ${APP_MESSAGES.summarySelectedModsLabel} ${C.CYAN}${selectedModNames}`
  );
  println(`${C.LIGHT_CYAN} ${SERVER_MESSAGES.roundSummaryFooter}`);
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
        `${C.LIGHT_YELLOW} * ${SERVER_MESSAGES.retryNotice(pendingRetryNotice)}`
      );
      printRoundConfigSummary(
        activePlan,
        currentPort,
        SERVER_MESSAGES.roundSummaryRetryTitle
      );
      pendingRetryNotice = null;
    }

    println();
    println(
      `${C.LIGHT_CYAN} ${SERVER_MESSAGES.roundLaunchHeader(round, currentPort)}`
    );
    println(
      `${C.LIGHT_WHITE} * ${SERVER_MESSAGES.roundMapLabel} ${C.LIGHT_GREEN}${activePlan.mapLabel} (${activePlan.mapId})  ${C.LIGHT_WHITE}${SERVER_MESSAGES.roundPortLabel} ${C.LIGHT_GREEN}${currentPort}`
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
          signal,
          onDetached: (reason: string): void => {
            if (!signal.aborted) {
              println(
                `${C.LIGHT_YELLOW} * ${SERVER_MESSAGES.fridaDetached(reason)}`
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
        println(`${C.RED} * ${SERVER_MESSAGES.startupInjectError(msg)}`);
        println(
          `${C.RED} * ${SERVER_MESSAGES.startupRetryAfter(formatSeconds(STARTUP_RETRY_DELAY_MS))}`
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
        `${C.LIGHT_CYAN} * ${SERVER_MESSAGES.monitoringServer(formatSeconds(config.checkIntervalMs))}`
      );
      println(`${C.LIGHT_BLACK} * ${SERVER_MESSAGES.exitHint}`);

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
                `${C.LIGHT_GREEN} * ${SERVER_MESSAGES.udpReadyConfirmed(shippingPid, currentPort)}`
              );
            }
            udpMissingStreak = 0;
          } else if (udpEverObserved) {
            udpMissingStreak += 1;
            if (udpMissingStreak >= UDP_MISSING_STREAK_LIMIT) {
              println(
                `${C.LIGHT_YELLOW} * ${SERVER_MESSAGES.udpStoppedRestarting(currentPort)}`
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
      if (shippingPid !== null && isProcessAlive(shippingPid)) {
        if (!signal.aborted) {
          // Give a normally-exiting server a brief grace window before force-terminating
          // so a detached zombie process can never remain alive holding consolePatchPort
          await sleepAbortable(DETACH_EXIT_GRACE_MS, signal);
        }
        if (isProcessAlive(shippingPid)) {
          terminateProcessByPid(shippingPid);
        }
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
      `${C.LIGHT_YELLOW} * ${SERVER_MESSAGES.serverExitDetectedPrefix} ${C.LIGHT_WHITE}${prevPort} -> ${C.LIGHT_GREEN}${currentPort}`
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
        SERVER_MESSAGES.roundSummaryUpdatedTitle
      );
    } else {
      resetServerTabRound();
      printRoundConfigSummary(
        activePlan,
        currentPort,
        SERVER_MESSAGES.roundSummaryContinuedTitle
      );
    }
  }
}
