export interface TerminalMessages {
  readonly pressEnterToExit: string;
  readonly postMatchCountdownPrefix: string;
  readonly postMatchCountdownMid: string;
  readonly postMatchCountdownReconfigure: string;
  readonly postMatchCountdownRestart: string;
  readonly tab1Title: (portSuffix: string) => string;
  readonly tab2Title: string;
  readonly tabSwitchHint: string;
  readonly serverScrollBadge: (offset: number) => string;
  readonly webSettingsHeader: string;
  readonly webTcpPortLabel: string;
  readonly webAutoBroadcastLabel: string;
  readonly toggleOn: string;
  readonly toggleOff: string;
  readonly webStartedHeader: string;
  readonly webStartedFooter: string;
  readonly publicUrlLabel: string;
  readonly localUrlLabel: string;
  readonly pskNoticeLead: string;
  readonly pskNoticeBody: string;
  readonly pskNoticeWarn: string;
  readonly internalBridgePortSummary: (port: number, broadcastText: string) => string;
  readonly copyPublicBtnLabel: string;
  readonly copyLocalBtnLabel: string;
  readonly publicUrlShortLabel: string;
  readonly localUrlShortLabel: string;
  readonly copySuccessFeedback: (label: string) => string;
  readonly copyFailureFeedback: string;
  readonly copyLogCategory: string;
  readonly copyLogSuccess: (label: string) => string;
  readonly copyLogFailure: (label: string) => string;
  readonly copyGuideHint: string;
  readonly serverPendingInputHint: string;
  readonly switchToServerHint: string;
  readonly webLogBarTitle: (totalLines: number, maxLines: number) => string;
  readonly webLogBarScrolled: (scrollPos: number) => string;
  readonly webLogBarControlsHint: string;
  readonly webLogEmptyPlaceholder: string;
}

export const TERMINAL_MESSAGES: TerminalMessages = {
  pressEnterToExit: "Enterキーを押して終了してください...",
  postMatchCountdownPrefix: "自動再起動まで残り",
  postMatchCountdownMid: "秒... (",
  postMatchCountdownReconfigure: ": マップ・MOD再設定 /",
  postMatchCountdownRestart: ": 今すぐ再起動)",
  tab1Title: (portSuffix: string): string => ` [1] Webコンソール${portSuffix} `,
  tab2Title: " [2] サーバー・試合設定 ",
  tabSwitchHint: "  [←/→・クリック切替]",
  serverScrollBadge: (offset: number): string =>
    ` ▲ ${offset}行上を表示中 (ホイール/PgDnで最新へ) `,
  webSettingsHeader:
    "------------------- [Webコンソール設定] -------------------",
  webTcpPortLabel: "TCPポート:",
  webAutoBroadcastLabel: "/ 自動ブロードキャスト:",
  toggleOn: "オン",
  toggleOff: "オフ",
  webStartedHeader:
    "==================== Webコンソール起動 ====================",
  webStartedFooter:
    "===========================================================",
  publicUrlLabel: "外部URL:",
  localUrlLabel: "内部URL:",
  pskNoticeLead: "URL末尾の #PSK は事前共有鍵(AES-256-GCM)です。",
  pskNoticeBody: "全API通信が密文で保護されます。",
  pskNoticeWarn: "信用していない人に渡さないで下さい",
  internalBridgePortSummary: (port: number, broadcastText: string): string =>
    `内部連携ポート(console_win64.js): 127.0.0.1:${port} / 自動ブロードキャスト: ${broadcastText}`,
  copyPublicBtnLabel: " 外部URLコピー (P) ",
  copyLocalBtnLabel: " 内部URLコピー (L) ",
  publicUrlShortLabel: "外部URL",
  localUrlShortLabel: "内部URL",
  copySuccessFeedback: (label: string): string =>
    ` * ✓ ${label}をクリップボードにコピーしました!`,
  copyFailureFeedback:
    " * クリップボードへのコピーに失敗しました (Shift+ドラッグで選択してください)",
  copyLogCategory: "コピー",
  copyLogSuccess: (label: string): string =>
    `${label}をクリップボードにコピーしました`,
  copyLogFailure: (label: string): string =>
    `${label}のクリップボードコピーに失敗`,
  copyGuideHint:
    "ボタンクリックまたは P / L キーでURLをコピー (手動選択は Shift+ドラッグ)",
  serverPendingInputHint:
    "※「サーバー・試合設定」タブで入力・操作待ちです (→キーまたはタブクリックで切替)",
  switchToServerHint:
    "→ キーまたは「サーバー・試合設定」タブをクリックしてサーバー画面へ切替",
  webLogBarTitle: (totalLines: number, maxLines: number): string =>
    ` [操作ログ ${totalLines}/${maxLines}] `,
  webLogBarScrolled: (scrollPos: number): string =>
    ` ▲ ${scrollPos}行上を表示中 (ホイール/PgDn/Endで最新へ) `,
  webLogBarControlsHint: " [↑/↓・ホイール・PgUp/PgDn・End] ",
  webLogEmptyPlaceholder: "(操作ログはまだありません)",
};
