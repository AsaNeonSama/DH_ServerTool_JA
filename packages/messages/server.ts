export interface ServerMessages {
  readonly opAborted: (label: string) => string;
  readonly opTimedOut: (label: string, timeoutMs: number) => string;
  readonly opFailed: (label: string) => string;
  readonly injectionAborted: string;
  readonly attachAborted: string;
  readonly attachLabel: (pid: number) => string;
  readonly attachFailedFallback: string;
  readonly launchAborted: string;
  readonly spawningSuspended: (executable: string, port: number) => string;
  readonly spawnLabel: string;
  readonly spawnedProcessPrefix: string;
  readonly spawnedProcessSuffix: string;
  readonly consoleBootstrapCreateLabel: string;
  readonly consoleBootstrapLoadLabel: string;
  readonly consoleBootstrapUnloadLabel: string;
  readonly patchInjectionAborted: string;
  readonly processDetachedDuringInjection: (reason: string) => string;
  readonly injectingPatchPrefix: (index: number, total: number) => string;
  readonly scriptCreateLabel: (fileName: string) => string;
  readonly scriptLoadLabel: (fileName: string) => string;
  readonly patchInitError: (fileName: string, errorText: string) => string;
  readonly injectedPatchDone: (index: number, total: number, label: string) => string;
  readonly resumeAborted: string;
  readonly resumeLabel: (pid: number) => string;
  readonly allPatchesInjectedResumed: (count: number) => string;
  readonly scriptUnloadLabel: string;
  readonly sessionDetachLabel: string;
  readonly processKillLabel: string;
  readonly patchLogHeader: (timestamp: string, port: number, patchNames: string) => string;
  readonly retryNotice: (errorText: string) => string;
  readonly roundSummaryRetryTitle: string;
  readonly roundSummaryUpdatedTitle: string;
  readonly roundSummaryContinuedTitle: string;
  readonly roundSummaryFooter: string;
  readonly roundSummaryCurrentPort: (port: number) => string;
  readonly roundLaunchHeader: (round: number, port: number) => string;
  readonly roundMapLabel: string;
  readonly roundPortLabel: string;
  readonly fridaDetached: (reason: string) => string;
  readonly startupInjectError: (errorText: string) => string;
  readonly startupRetryAfter: (delayText: string) => string;
  readonly monitoringServer: (intervalText: string) => string;
  readonly exitHint: string;
  readonly udpReadyConfirmed: (pid: number, port: number) => string;
  readonly udpStoppedRestarting: (port: number) => string;
  readonly serverExitDetectedPrefix: string;
}

export const SERVER_MESSAGES: ServerMessages = {
  opAborted: (label: string): string => `${label} が中断されました`,
  opTimedOut: (label: string, timeoutMs: number): string =>
    `${label} がタイムアウトしました (${timeoutMs}ms)`,
  opFailed: (label: string): string => `${label} に失敗しました`,
  injectionAborted: "注入処理が中断されました",
  attachAborted: "Fridaアタッチが中断されました",
  attachLabel: (pid: number): string => `Fridaアタッチ (PID: ${pid})`,
  attachFailedFallback: "Frida attach failed",
  launchAborted: "サーバー起動が中断されました",
  spawningSuspended: (executable: string, port: number): string =>
    `${executable} を一時停止状態で直接起動中 (ポート: ${port})...`,
  spawnLabel: "サーバープロセス生成",
  spawnedProcessPrefix: "プロセス生成完了 (PID:",
  spawnedProcessSuffix: ") — パッチ注入を開始します",
  consoleBootstrapCreateLabel: "コンソール初期化スクリプト生成",
  consoleBootstrapLoadLabel: "コンソール初期化スクリプト実行",
  consoleBootstrapUnloadLabel: "コンソール初期化スクリプト解放",
  patchInjectionAborted: "パッチ注入が中断されました",
  processDetachedDuringInjection: (reason: string): string =>
    `プロセスから切断されたため注入を中断しました (${reason})`,
  injectingPatchPrefix: (index: number, total: number): string =>
    `[${index}/${total}] 注入中:`,
  scriptCreateLabel: (fileName: string): string => `スクリプト生成 (${fileName})`,
  scriptLoadLabel: (fileName: string): string => `スクリプト注入 (${fileName})`,
  patchInitError: (fileName: string, errorText: string): string =>
    `パッチ初期化エラー (${fileName}): ${errorText}`,
  injectedPatchDone: (index: number, total: number, label: string): string =>
    `[${index}/${total}] 注入完了: ${label}`,
  resumeAborted: "サーバー再開が中断されました",
  resumeLabel: (pid: number): string => `サーバー再開 (PID: ${pid})`,
  allPatchesInjectedResumed: (count: number): string =>
    `全 ${count} 件のMODパッチの注入完了 -> サーバー実行を再開しました!`,
  scriptUnloadLabel: "スクリプト解放",
  sessionDetachLabel: "セッション切断",
  processKillLabel: "プロセス終了",
  patchLogHeader: (timestamp: string, port: number, patchNames: string): string =>
    `=== DH_ServerTool_JA Patch Runtime Log (${timestamp} / Port: ${port}) ===\nPatches: ${patchNames}\n`,
  retryNotice: (errorText: string): string =>
    `前回起動失敗のためポート切替再試行 (${errorText})`,
  roundSummaryRetryTitle:
    "======================= 起動設定まとめ =======================",
  roundSummaryUpdatedTitle:
    "======================= 更新後の設定 =======================",
  roundSummaryContinuedTitle:
    "================= 起動設定 (前回設定を継続) =================",
  roundSummaryFooter:
    "============================================================",
  roundSummaryCurrentPort: (port: number): string => `(今回: ${port})`,
  roundLaunchHeader: (round: number, port: number): string =>
    `================= サーバー起動 (#${round} / ポート: ${port}) =================`,
  roundMapLabel: "マップ:",
  roundPortLabel: "ポート:",
  fridaDetached: (reason: string): string =>
    `Fridaセッション切断を検知しました (${reason})`,
  startupInjectError: (errorText: string): string =>
    `サーバー起動・注入エラー: ${errorText}`,
  startupRetryAfter: (delayText: string): string =>
    `${delayText}後にポートを切り替えて再試行します...`,
  monitoringServer: (intervalText: string): string =>
    `サーバー監視中 (${intervalText}間隔 / 終了時に自動ポート切替・再起動します)...`,
  exitHint: "終了する場合は Ctrl+C を押してください",
  udpReadyConfirmed: (pid: number, port: number): string =>
    `サーバー起動完了確認 (PID: ${pid}, UDP:${port} リッスン中)`,
  udpStoppedRestarting: (port: number): string =>
    `ポート ${port} のリッスン停止を検知しました。サーバーを再起動します...`,
  serverExitDetectedPrefix: "サーバーの終了を検知しました! 次回起動ポート:",
};
