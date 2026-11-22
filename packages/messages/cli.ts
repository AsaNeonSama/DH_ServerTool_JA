import type { MapChoiceKey } from "../types";

export interface CliSettingLabels {
  readonly maxPlayers: string;
  readonly daysBeforeBlizzard: string;
  readonly dayMinutes: string;
  readonly predatorDamage: string;
  readonly coldIntensity: string;
  readonly hungerRate: string;
  readonly coalBurnRate: string;
  readonly thralls: string;
}

export interface CliMessages {
  readonly mapLabels: Readonly<Record<MapChoiceKey, string>>;
  readonly webConsoleSectionHeader: string;
  readonly serverSetupSectionHeader: string;
  readonly reconfigureSectionHeader: string;
  readonly webPortPrompt: string;
  readonly portRangeError: (min: number, max: number) => string;
  readonly webPortConflictError: (patchPort: number) => string;
  readonly autoBroadcastPromptPrefix: string;
  readonly autoBroadcastChoices: string;
  readonly serverPortPromptPrefix: string;
  readonly serverPortDuplicateError: string;
  readonly serverPortWebConflictError: (webPort: number, patchPort: number) => string;
  readonly serverPortsSelectedLabel: string;
  readonly mapPromptPrefix: string;
  readonly mapPromptChoices: string;
  readonly serverConfigPromptPrefix: string;
  readonly serverConfigChoices: string;
  readonly settingLabels: CliSettingLabels;
  readonly invalidChoiceRetry: string;
  readonly invalidChoiceShort: string;
  readonly invalidChoiceList: (items: string) => string;
  readonly defaultModsLabel: string;
  readonly modSelectionGuideBeforeY: string;
  readonly modSelectionGuideBetweenYAndC: string;
  readonly modSelectionGuideAfterC: string;
  readonly modChoicePrompt: string;
  readonly modModeSuffix: string;
  readonly modConflictLegendPrefix: string;
  readonly modSelectionCleared: string;
  readonly modConflictDetail: (optionText: string, targets: string) => string;
  readonly modAddedPrefix: string;
  readonly modDuplicatePrefix: string;
  readonly modCurrentSelection: (names: string) => string;
  readonly modConflictSkippedPrefix: string;
  readonly reconfigurePreservedNotice: (port1: number, port2: number) => string;
  readonly reconfigureInterruptedNotice: (errorText: string) => string;
}

export const CLI_MESSAGES: CliMessages = {
  mapLabels: {
    "1": "山頂",
    "2": "広漠",
    "3": "進入路",
  },
  webConsoleSectionHeader:
    "-------------------- [Webコンソール設定] --------------------",
  serverSetupSectionHeader:
    "-------------------- [サーバー・試合設定] --------------------",
  reconfigureSectionHeader:
    "================== マップ・MOD再設定モード ==================",
  webPortPrompt: "Webコンソールに使うTCPポートを入れてください",
  portRangeError: (min: number, max: number): string =>
    `無効な入力です。${min}〜${max}の数値を入力してください!`,
  webPortConflictError: (patchPort: number): string =>
    `コンソールパッチ内部ポート(${patchPort})と重複しています。別のポートを指定してください!`,
  autoBroadcastPromptPrefix: "自動ブロードキャストスイッチ",
  autoBroadcastChoices: "1:オン, 0:オフ",
  serverPortPromptPrefix: "サーバーに使うポートを入れてください",
  serverPortDuplicateError:
    "1つ目と2つ目のポートは別の番号を指定してください!",
  serverPortWebConflictError: (webPort: number, patchPort: number): string =>
    `Webコンソール関連ポート(${webPort}, ${patchPort})と重複しています。別のポートを指定してください!`,
  serverPortsSelectedLabel: "サーバーポート:",
  mapPromptPrefix: "マップを設定してください",
  mapPromptChoices: "1:山頂, 2:広漠, 3:進入路",
  serverConfigPromptPrefix: "サーバーの配置を設定してください",
  serverConfigChoices: "1:デフォルト設定, 2:設定を編集する",
  settingLabels: {
    maxPlayers: "最大人数設定",
    daysBeforeBlizzard: "暴風雪前の日数設定",
    dayMinutes: "一日の時間設定",
    predatorDamage: "野生動物ダメージ設定",
    coldIntensity: "寒さ強度設定",
    hungerRate: "ハングリー強度設定",
    coalBurnRate: "石炭燃焼率設定",
    thralls: "傀儡人数設定",
  },
  invalidChoiceRetry: "規格外の選択肢を入力しました、やり直してください!",
  invalidChoiceShort: "規格外の選択肢です!",
  invalidChoiceList: (items: string): string => `規格外の選択肢です: ${items}`,
  defaultModsLabel: "デフォルトMOD(自動注入):",
  modSelectionGuideBeforeY:
    "下記のナンバーで機能を選択してください(カンマ区切りで複数選択可 例: 1,2,7 /",
  modSelectionGuideBetweenYAndC: "で決定 /",
  modSelectionGuideAfterC: "でクリア):",
  modChoicePrompt: "選択肢を入力してください:",
  modModeSuffix: " [セット]",
  modConflictLegendPrefix: "同時選択不可(排他):",
  modSelectionCleared: "選択済みのスクリプトをクリアしました。",
  modConflictDetail: (optionText: string, targets: string): string =>
    `${optionText} (${targets}と同時選択不可)`,
  modAddedPrefix: "追加しました:",
  modDuplicatePrefix: "選択済みのためスキップ:",
  modCurrentSelection: (names: string): string => `(選択中: ${names})`,
  modConflictSkippedPrefix: "排他(同時選択不可)のためスキップ:",
  reconfigurePreservedNotice: (port1: number, port2: number): string =>
    `Webコンソールとサーバーポート設定(${port1} <-> ${port2})はそのまま維持されます`,
  reconfigureInterruptedNotice: (errorText: string): string =>
    `再設定が中断されました (${errorText})。前回設定のまま再起動します。`,
};
