export interface DefaultModLabels {
  readonly console: string;
  readonly spellChargeFix: string;
  readonly deathOfflineAnnouncement: string;
  readonly networkFix: string;
}

export interface BoneDaggerScriptLabels {
  readonly main: string;
  readonly dropTable: string;
  readonly inventory: string;
}

export interface SelectableModLabels {
  readonly randRole: string;
  readonly spellCastStart: string;
  readonly resourcesSpawnerDeparture: string;
  readonly resourcesSpawnerExpanse: string;
  readonly spellCustom: string;
  readonly madmateNormalNolimit: string;
  readonly boneDaggerMode: string;
  readonly boneDaggerScripts: BoneDaggerScriptLabels;
  readonly questSystem: string;
  readonly addStorage: string;
  readonly whetstoneAnyItem: string;
  readonly arrowFullRecovery: string;
  readonly blizzardArmoryOpen: string;
  readonly gunAndBow: string;
  readonly startArmoryOpen: string;
  readonly vomitModify: string;
  readonly northGhost: string;
  readonly grenade: string;
  readonly moonJump: string;
  readonly spellNoCd: string;
  readonly noDeath: string;
  readonly sightseeingDeparture: string;
  readonly sightseeingExpanse: string;
}

export interface ModMessages {
  readonly defaults: DefaultModLabels;
  readonly selectable: SelectableModLabels;
}

export const MOD_MESSAGES: ModMessages = {
  defaults: {
    console: "Webコンソール連携",
    spellChargeFix: "スペルチャージ修正",
    deathOfflineAnnouncement: "オフラインアナウンス",
    networkFix: "ネットワーク・再接続修正",
  },
  selectable: {
    randRole: "役職ランダム",
    spellCastStart: "開幕沈黙",
    resourcesSpawnerDeparture: "マップ補強(山頂)",
    resourcesSpawnerExpanse: "マップ補強(広漠)",
    spellCustom: "五サボタージュ",
    madmateNormalNolimit: "狂人モード",
    boneDaggerMode: "骨の短剣モード",
    boneDaggerScripts: {
      main: "骨の短剣モード (短剣本体)",
      dropTable: "骨の短剣モード (ドロップテーブル)",
      inventory: "骨の短剣モード (初期装備設定)",
    },
    questSystem: "クエスト報酬",
    addStorage: "全員バックパック",
    whetstoneAnyItem: "万能砥石",
    arrowFullRecovery: "矢回収100%",
    blizzardArmoryOpen: "暴風雪マスケット",
    gunAndBow: "銃・弓同時所持",
    startArmoryOpen: "開幕マスケット",
    vomitModify: "毒耐性",
    northGhost: "ミステリーモード",
    grenade: "グレネード",
    moonJump: "八艘飛び",
    spellNoCd: "スペルチャージ無限",
    noDeath: "無限復活",
    sightseeingDeparture: "観光モード(山頂)",
    sightseeingExpanse: "観光モード(広漠)",
  },
};
