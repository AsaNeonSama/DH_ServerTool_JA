export interface ConfigSummaryParams {
  readonly serverPort1: number;
  readonly serverPort2: number;
  readonly webConsolePort: number;
  readonly consolePatchPort: number;
  readonly checkIntervalText: string;
  readonly restartWaitSeconds: number;
}

export interface AppMessages {
  readonly authorLabel: string;
  readonly authorName: string;
  readonly brandBuild: string;
  readonly brandWith: string;
  readonly brandAntigravity: string;
  readonly missingServerDirError: string;
  readonly publicIpLabel: string;
  readonly publicIpFetchFailed: (errorText: string) => string;
  readonly missingShippingExecutable: (path: string) => string;
  readonly legacyFridaDllDetected: (fileName: string, fullPath: string) => string;
  readonly legacyFridaDllAdvice: (processName: string) => string;
  readonly configSummary: (params: ConfigSummaryParams) => string;
  readonly configCreated: (relativePath: string) => string;
  readonly configLoadFailed: (errorText: string) => string;
  readonly webConsoleStartFailed: (port: number, errorText: string) => string;
  readonly shutdownSignalReceived: string;
  readonly summaryHeader: string;
  readonly summaryFooter: string;
  readonly summaryMapLabel: string;
  readonly summaryServerPortLabel: string;
  readonly summaryWebConsoleLabel: string;
  readonly summaryInternalPatchPort: (port: number) => string;
  readonly summaryDefaultModsLabel: string;
  readonly summarySelectedModsLabel: string;
  readonly noneLabel: string;
  readonly exitedCleanly: string;
  readonly unexpectedErrorHeader: string;
}

export const APP_MESSAGES: AppMessages = {
  authorLabel: "ソフト作者:",
  authorName: "アサネオン",
  brandBuild: "Build",
  brandWith: "With",
  brandAntigravity: "Antigravity",
  missingServerDirError:
    "本ソフトをサーバーのフォルダの下に入れた後で再起動してください!",
  publicIpLabel: "パブリックIP:",
  publicIpFetchFailed: (errorText: string): string =>
    `パブリックIPの取得に失敗しました: ${errorText}`,
  missingShippingExecutable: (path: string): string =>
    `サーバー実行ファイルが見つかりません: ${path}`,
  legacyFridaDllDetected: (fileName: string, fullPath: string): string =>
    `警告: 旧パッチ用の ${fileName} (${fullPath}) が検出されました。`,
  legacyFridaDllAdvice: (processName: string): string =>
    `原生サーバーで安定動作させるため、未改造の ${processName} を使用し frida.dll / frida.config / Patches を削除してください。`,
  configSummary: (params: ConfigSummaryParams): string =>
    `config.json: サーバーポート=${params.serverPort1}/${params.serverPort2}, Webコンソール=${params.webConsolePort}, パッチ連携=${params.consolePatchPort}, 監視間隔=${params.checkIntervalText}, 再設定待機=${params.restartWaitSeconds}s, 注入方式=Frida直接起動注入(spawn)`,
  configCreated: (relativePath: string): string =>
    `デフォルト設定ファイル (${relativePath}) を作成しました`,
  configLoadFailed: (errorText: string): string =>
    `config.json の読み込みに失敗しました。デフォルト設定を使用します: ${errorText}`,
  webConsoleStartFailed: (port: number, errorText: string): string =>
    `Webコンソールの起動に失敗しました (ポート ${port}): ${errorText}`,
  shutdownSignalReceived:
    "終了シグナルを受信しました。サーバーとWebコンソールを停止しています...",
  summaryHeader: "======================= 起動設定まとめ =======================",
  summaryFooter: "==============================================================",
  summaryMapLabel: "マップ:        ",
  summaryServerPortLabel: "サーバーポート:",
  summaryWebConsoleLabel: "Webコンソール: ",
  summaryInternalPatchPort: (port: number): string => `(内部パッチポート: ${port})`,
  summaryDefaultModsLabel: "自動注入MOD:   ",
  summarySelectedModsLabel: "選択済みMOD:   ",
  noneLabel: "なし",
  exitedCleanly: "正常に終了しました。",
  unexpectedErrorHeader: "予期しないエラーが発生しました:",
};
