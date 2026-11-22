export interface WebConsoleLogCategories {
  readonly auth: string;
  readonly error: string;
  readonly command: string;
  readonly status: string;
  readonly item: string;
  readonly storage: string;
  readonly notice: string;
  readonly broadcast: string;
  readonly kick: string;
  readonly startup: string;
  readonly shutdown: string;
}

export interface WebConsoleResultBadges {
  readonly sent: string;
  readonly partialFailure: string;
  readonly sendFailed: (suffix: string) => string;
  readonly invalid: (suffix: string) => string;
}

export interface WebConsoleApiMessages {
  readonly success: string;
  readonly networkParamError: string;
  readonly serverExecutionError: string;
  readonly invalidValueError: string;
  readonly highValueWarning: string;
  readonly broadcastPartialError: string;
}

export interface WebConsoleMessages {
  readonly playerRoleNames: Readonly<Record<string, string>>;
  readonly actorDisplayNames: Readonly<Record<string, string>>;
  readonly api: WebConsoleApiMessages;
  readonly logCategories: WebConsoleLogCategories;
  readonly resultBadges: WebConsoleResultBadges;
  readonly logDetails: {
    readonly unauthenticatedRejected: (clientIp: string, pathname: string) => string;
    readonly invalidRequestBody: (clientIp: string, pathname: string) => string;
    readonly decryptFailed: (clientIp: string, pathname: string) => string;
    readonly invalidJsonParams: (clientIp: string, pathname: string) => string;
    readonly internalRouteError: (clientIp: string, pathname: string, msg: string) => string;
    readonly loginFailedInvalidBody: (clientIp: string) => string;
    readonly loginFailedPskMismatch: (clientIp: string) => string;
    readonly loginFailedChallengeInvalid: (clientIp: string) => string;
    readonly loginSucceeded: (clientIp: string) => string;
    readonly loginFailedInternal: (clientIp: string) => string;
    readonly invalidParams: string;
    readonly invalidNumberReason: string;
    readonly invalidCountReason: string;
    readonly invalidValueReason: string;
    readonly boneDaggerAddedSuffix: string;
    readonly storageCapacityLabel: string;
    readonly broadcastAllTarget: string;
    readonly kickVerb: string;
    readonly startupDetail: (
      webPort: number,
      patchPort: number,
      broadcastText: string
    ) => string;
    readonly shutdownDetail: string;
  };
  readonly broadcasts: {
    readonly summon: (roleName: string, target: string) => string;
    readonly unsetThrall: (roleName: string) => string;
    readonly setThrall: (roleName: string) => string;
    readonly spellCharge: (roleName: string) => string;
    readonly stopBlizzard: (roleName: string) => string;
    readonly startBlizzard: (roleName: string) => string;
    readonly resetTimeScale: (roleName: string) => string;
    readonly accelerateTimeScale: (roleName: string) => string;
    readonly disableGodMode: (roleName: string) => string;
    readonly enableGodMode: (roleName: string) => string;
    readonly modifiedStats: (roleName: string) => string;
    readonly acquiredItem: (roleName: string, itemLabel: string, count: number) => string;
    readonly storageLimitChanged: (roleName: string, limit: number) => string;
    readonly kicked: (roleName: string) => string;
  };
}

export const WEB_CONSOLE_MESSAGES: WebConsoleMessages = {
  playerRoleNames: {
    Captain: "船長",
    Chaplain: "牧師",
    Cook: "料理人",
    Doctor: "医者",
    Engineer: "エンジニア",
    Hunter: "猟師",
    Marine: "海兵",
    Navigator: "航海士",
  },
  actorDisplayNames: {
    BP_Prey_Rat_C: "ネズミ",
    BP_Prey_Rabbit_C: "ウサギ",
    BP_Prey_Seal_C: "アザラシ",
    BP_Predator_Wolf_C: "狼",
    BP_Predator_Bear_C: "熊",
    BP_Cannibal_C: "人喰い人種",
    BP_Cannibal_Crawler_C: "人喰い人種",
    BP_Cannibal_Shambler_C: "人喰い人種",
    BP_HumanCharacter_C: "海兵",
    BP_Totem_C: "トーテム",
    BP_ExplosionActor_C: "爆裂魔法",
    BP_Explosion_Nitro_C: "大爆裂魔法・チャージ 120 %",
    "/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Inventory.BP_Flintlock_Inventory_C": "拳銃",
    "/Game/Blueprints/Inventory/Musket/BP_Musket_Inventory.BP_Musket_Inventory_C": "マスケット",
    "/Game/Blueprints/Inventory/Bow/BP_Bow_Inventory.BP_Bow_Inventory_C": "弓",
    "/Game/Blueprints/Inventory/Flint/BP_GunParts_Inventory.BP_GunParts_Inventory_C": "拳銃パーツ",
    "/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Ammo_Inventory.BP_Flintlock_Ammo_Inventory_C": "銃弾",
    "/Game/Blueprints/Inventory/Bow/BP_Arrows_Inventory.BP_Arrows_Inventory_C": "弓矢",
    "/Game/Blueprints/Inventory/Sword/BP_Sword_Inventory.BP_Sword_Inventory_C": "サーベル",
    "/Game/Blueprints/Inventory/WoodAxe/BP_WoodAxe_Inventory.BP_WoodAxe_Inventory_C": "木の斧",
    "/Game/Blueprints/Inventory/IceAxe/BP_IceAxe_Inventory.BP_IceAxe_Inventory_C": "ピッケル",
    "/Game/Blueprints/Inventory/Cleaver/BP_Cleaver_Inventory.BP_Cleaver_Inventory_C": "包丁",
    "/Game/Blueprints/Inventory/Shovel/BP_Shovel_Inventory.BP_Shovel_Inventory_C": "シャベル",
    "/Game/Blueprints/Inventory/Stick/BP_Stick_Inventory.BP_Stick_Inventory_C": "木",
    "/Game/Blueprints/Inventory/Rock/BP_Rock_Inventory.BP_Rock_Inventory_C": "石",
    "/Game/Blueprints/Inventory/Metals/BP_IronIngot_Inventory.BP_IronIngot_Inventory_C": "スクラップ",
    "/Game/Blueprints/Inventory/Metals/BP_Nails_Inventory.BP_Nails_Inventory_C": "釘",
    "/Game/Blueprints/Inventory/Metals/BP_LeadIngot_Inventory.BP_LeadIngot_Inventory_C": "鉛",
    "/Game/Blueprints/Inventory/Coal/BP_Coal_Inventory.BP_Coal_Inventory_C": "石炭",
    "/Game/Blueprints/Inventory/Flintlock/BP_Gunpowder_Inventory.BP_Gunpowder_Inventory_C": "火薬",
    "/Game/Blueprints/Inventory/AnimalParts/BP_Sinew_Inventory.BP_Sinew_Inventory_C": "腱",
    "/Game/Blueprints/Inventory/AnimalParts/BP_WolfPelt_Inventory.BP_WolfPelt_Inventory_C": "毛皮",
    "/Game/Blueprints/Inventory/Tea/BP_Herbs_Inventory.BP_Herbs_Inventory_C": "ドライハーブ",
    "/Game/Blueprints/Inventory/Syringe/BP_Syringe_Inventory.BP_Syringe_Inventory_C": "注射器",
    "/Game/Blueprints/Inventory/Poison/BP_Antidote_Inventory.BP_Antidote_Inventory_C": "解毒剤",
    "/Game/Blueprints/Inventory/Syringe/BP_Inventory_Laudanum.BP_Inventory_Laudanum_C": "アヘンチンキ",
    "/Game/Blueprints/Inventory/Meat/BP_AnimalMeat_Inventory.BP_AnimalMeat_Inventory_C": "動物肉",
    "/Game/Blueprints/Inventory/Meat/BP_CookedMeat_Inventory.BP_CookedMeat_Inventory_C": "焼肉",
    "/Game/Blueprints/Inventory/Meat/BP_HumanMeat_Inventory.BP_HumanMeat_Inventory_C": "人肉",
    "/Game/Blueprints/Inventory/Meat/BP_BoneClub_Inventory.BP_BoneClub_Inventory_C": "骨の棒",
    "/Game/Blueprints/Inventory/AnimalParts/BP_Blubber_Inventory.BP_Blubber_Inventory_C": "脂",
    "/Game/Blueprints/Inventory/Meat/BP_Stew_Inventory.BP_Stew_Inventory_C": "シチュー",
    "/Game/Blueprints/Inventory/Tea/BP_Tea_Inventory.BP_Tea_Inventory_C": "お茶",
    "/Game/Blueprints/Inventory/Metals/BP_Whetstone_Inventory.BP_Whetstone_Inventory_C": "砥石",
    "/Game/Blueprints/Inventory/BearTrap/BP_BearTrap_Inventory.BP_BearTrap_Inventory_C": "熊罠",
    "/Game/Blueprints/Inventory/Spyglass/BP_Spyglass_Inventory.BP_Spyglass_Inventory_C": "望遠鏡",
    "/Game/Blueprints/Inventory/Lantern/BP_Lantern_Inventory.BP_Lantern_Inventory_C": "ランタン",
    "/Game/Blueprints/Inventory/Powderkeg/BP_CoalBarrel_Inventory.BP_CoalBarrel_Inventory_C": "石炭樽",
    "/Game/Blueprints/Inventory/Powderkeg/BP_Powderkeg_Inventory.BP_Powderkeg_Inventory_C": "火薬樽",
    "/Game/Blueprints/Inventory/Poison/BP_Poison_Inventory.BP_Poison_Inventory_C": "毒薬",
    "/Game/Blueprints/Environment/Nitro/BP_Nitro_Inventory.BP_Nitro_Inventory_C": "ニトログリセリン",
    "/Game/Blueprints/Inventory/LockPick/BP_SkeletonKey_Inventory.BP_SkeletonKey_Inventory_C": "骨の鍵",
    "/Game/Blueprints/Inventory/LockPick/BP_CaptainsKey_Inventory.BP_CaptainsKey_Inventory_C": "船長室の鍵",
    "/Game/Blueprints/Inventory/Armory/BP_Code_Inventory.BP_Code_Inventory_C": "武器庫の暗号",
    "/Game/Blueprints/Inventory/Quest/BP_Quest_Inventory.BP_Quest_Inventory_C": "任務の巻物",
    "/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C": "骨の短剣",
  },
  api: {
    success: "成功しました!",
    networkParamError:
      "ネットワーク問題で引数を取得できませんでした。もう一度試してください!",
    serverExecutionError:
      "サーバー側で問題が発生しました。試合開始後にもう一度試すか、サーバーの再起動・JSパッチをご確認ください!",
    invalidValueError: "数値エラー!",
    highValueWarning: "送信成功! 数値が高すぎるとエラーの可能性があります!",
    broadcastPartialError: "送信成功しましたが、自動ブロードキャストに失敗しました!",
  },
  logCategories: {
    auth: "認証",
    error: "エラー",
    command: "コマンド",
    status: "ステータス",
    item: "アイテム",
    storage: "ストレージ",
    notice: "通知",
    broadcast: "全体通知",
    kick: "キック",
    startup: "起動",
    shutdown: "停止",
  },
  resultBadges: {
    sent: "[送信済]",
    partialFailure: "[一部送信失敗]",
    sendFailed: (suffix: string): string => `[送信失敗${suffix}]`,
    invalid: (suffix: string): string => `[無効${suffix}]`,
  },
  logDetails: {
    unauthenticatedRejected: (clientIp: string, pathname: string): string =>
      `${clientIp} | 未認証リクエスト拒否 (${pathname})`,
    invalidRequestBody: (clientIp: string, pathname: string): string =>
      `${clientIp} | リクエスト本体不正 (${pathname})`,
    decryptFailed: (clientIp: string, pathname: string): string =>
      `${clientIp} | 暗号復号失敗 (${pathname})`,
    invalidJsonParams: (clientIp: string, pathname: string): string =>
      `${clientIp} | 不正なJSONパラメータ (${pathname})`,
    internalRouteError: (clientIp: string, pathname: string, msg: string): string =>
      `${clientIp} | 内部エラー (${pathname}): ${msg}`,
    loginFailedInvalidBody: (clientIp: string): string =>
      `${clientIp} | ログイン失敗 (リクエスト不正)`,
    loginFailedPskMismatch: (clientIp: string): string =>
      `${clientIp} | ログイン失敗 (PSK不一致)`,
    loginFailedChallengeInvalid: (clientIp: string): string =>
      `${clientIp} | ログイン失敗 (チャレンジ期限切れ/無効)`,
    loginSucceeded: (clientIp: string): string => `${clientIp} | ログイン成功`,
    loginFailedInternal: (clientIp: string): string =>
      `${clientIp} | ログイン失敗 (内部エラー)`,
    invalidParams: "パラメータ不正",
    invalidNumberReason: "不正な数値",
    invalidCountReason: "不正な個数",
    invalidValueReason: "不正な値",
    boneDaggerAddedSuffix: " (+骨の短剣)",
    storageCapacityLabel: "容量 ->",
    broadcastAllTarget: "全員",
    kickVerb: "をキック",
    startupDetail: (
      webPort: number,
      patchPort: number,
      broadcastText: string
    ): string =>
      `Webコンソール待機開始 (ポート: ${webPort} / 内部連携: 127.0.0.1:${patchPort} / 自動ブロードキャスト: ${broadcastText})`,
    shutdownDetail: "Webコンソールを停止しました",
  },
  broadcasts: {
    summon: (roleName: string, target: string): string =>
      `${roleName} が${target}を召喚しました`,
    unsetThrall: (roleName: string): string =>
      `${roleName} が船員に戻りました`,
    setThrall: (roleName: string): string =>
      `${roleName} が闇の傀儡に悪墜ちしました`,
    spellCharge: (roleName: string): string =>
      `${roleName} がスペールチャージしました`,
    stopBlizzard: (roleName: string): string =>
      `${roleName} が暴風雪を停止しました`,
    startBlizzard: (roleName: string): string =>
      `${roleName} が暴風雪を呼びました`,
    resetTimeScale: (roleName: string): string =>
      `${roleName} が世界を元に戻しました`,
    accelerateTimeScale: (roleName: string): string =>
      `${roleName} が世界を加速しました`,
    disableGodMode: (roleName: string): string =>
      `${roleName} が無敵状態を解除しました`,
    enableGodMode: (roleName: string): string =>
      `${roleName} が無敵状態になりました`,
    modifiedStats: (roleName: string): string =>
      `${roleName} が自分のステータスを操作しました`,
    acquiredItem: (roleName: string, itemLabel: string, count: number): string =>
      `${roleName} が ${itemLabel} ${count} を獲得しました`,
    storageLimitChanged: (roleName: string, limit: number): string =>
      `${roleName} のストレージ容量が ${limit} になりました`,
    kicked: (roleName: string): string => `${roleName} がキックされました`,
  },
};
