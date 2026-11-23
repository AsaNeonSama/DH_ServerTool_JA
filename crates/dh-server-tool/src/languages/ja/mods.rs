use crate::structs::languages::ModLabelCode;

#[must_use]
pub fn label(code: ModLabelCode) -> &'static str {
    match code {
        ModLabelCode::DefaultConsole => "Webコンソール連携",
        ModLabelCode::DefaultSpellChargeFix => "スペルチャージ修正",
        ModLabelCode::DefaultDeathOfflineAnnouncement => "オフラインアナウンス",
        ModLabelCode::DefaultNetworkFix => "ネットワーク接続保護・乱入防止",
        ModLabelCode::RandRole => "役職ランダム",
        ModLabelCode::SpellCastStart => "開幕沈黙",
        ModLabelCode::ResourcesSpawnerDeparture => "マップ補強(山頂)",
        ModLabelCode::ResourcesSpawnerExpanse => "マップ補強(広漠)",
        ModLabelCode::SpellCustom => "五サボタージュ",
        ModLabelCode::MadmateNormalNolimit => "狂人モード",
        ModLabelCode::BoneDaggerMode => "骨の短剣モード",
        ModLabelCode::BoneDaggerScriptMain => "骨の短剣モード (短剣本体)",
        ModLabelCode::BoneDaggerScriptDropTable => "骨の短剣モード (ドロップテーブル)",
        ModLabelCode::BoneDaggerScriptInventory => "骨の短剣モード (初期装備設定)",
        ModLabelCode::QuestSystem => "クエスト報酬",
        ModLabelCode::AddStorage => "全員バックパック",
        ModLabelCode::WhetstoneAnyItem => "万能砥石",
        ModLabelCode::ArrowFullRecovery => "矢回収100%",
        ModLabelCode::BlizzardArmoryOpen => "暴風雪マスケット",
        ModLabelCode::GunAndBow => "銃・弓同時所持",
        ModLabelCode::StartArmoryOpen => "開幕マスケット",
        ModLabelCode::VomitModify => "毒耐性",
        ModLabelCode::NorthGhost => "ミステリーモード",
        ModLabelCode::Grenade => "グレネード",
        ModLabelCode::MoonJump => "八艘飛び",
        ModLabelCode::SpellNoCd => "スペルチャージ無限",
        ModLabelCode::NoDeath => "無限復活",
        ModLabelCode::SightseeingDeparture => "観光モード(山頂)",
        ModLabelCode::SightseeingExpanse => "観光モード(広漠)",
    }
}
