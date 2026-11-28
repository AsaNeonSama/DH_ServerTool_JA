use crate::structs::languages::{
    ApiCode, BroadcastEvent, ConsoleLogDetailCode, LogCategoryCode, ResultBadgeCode,
};

#[must_use]
pub fn player_role_name(role: &str) -> Option<&'static str> {
    match role {
        "Captain" => Some("船長"),
        "Chaplain" => Some("牧師"),
        "Cook" => Some("料理人"),
        "Doctor" => Some("医者"),
        "Engineer" => Some("エンジニア"),
        "Hunter" => Some("猟師"),
        "Marine" => Some("海兵"),
        "Navigator" => Some("航海士"),
        _ => None,
    }
}

#[must_use]
pub fn actor_display_name(key: &str) -> Option<&'static str> {
    match key {
        "BP_Prey_Rat_C" => Some("ネズミ"),
        "BP_Prey_Rabbit_C" => Some("ウサギ"),
        "BP_Prey_Seal_C" => Some("アザラシ"),
        "BP_Predator_Wolf_C" => Some("狼"),
        "BP_Predator_Bear_C" => Some("熊"),
        "BP_Cannibal_C" | "BP_Cannibal_Crawler_C" | "BP_Cannibal_Shambler_C" => {
            Some("人喰い人種")
        }
        "BP_HumanCharacter_C" => Some("海兵 (船員)"),
        "BP_HumanCharacter_Thrall_C" => Some("海兵 (傀儡)"),
        "BP_Totem_C" => Some("トーテム"),
        "BP_ExplosionActor_C" => Some("爆裂魔法"),
        "BP_Explosion_Nitro_C" => Some("大爆裂魔法・チャージ 120 %"),
        "/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Inventory.BP_Flintlock_Inventory_C" => {
            Some("拳銃")
        }
        "/Game/Blueprints/Inventory/Musket/BP_Musket_Inventory.BP_Musket_Inventory_C" => {
            Some("マスケット")
        }
        "/Game/Blueprints/Inventory/Bow/BP_Bow_Inventory.BP_Bow_Inventory_C" => Some("弓"),
        "/Game/Blueprints/Inventory/Flint/BP_GunParts_Inventory.BP_GunParts_Inventory_C" => {
            Some("拳銃パーツ")
        }
        "/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Ammo_Inventory.BP_Flintlock_Ammo_Inventory_C" => {
            Some("銃弾")
        }
        "/Game/Blueprints/Inventory/Bow/BP_Arrows_Inventory.BP_Arrows_Inventory_C" => {
            Some("弓矢")
        }
        "/Game/Blueprints/Inventory/Sword/BP_Sword_Inventory.BP_Sword_Inventory_C" => {
            Some("サーベル")
        }
        "/Game/Blueprints/Inventory/WoodAxe/BP_WoodAxe_Inventory.BP_WoodAxe_Inventory_C" => {
            Some("木の斧")
        }
        "/Game/Blueprints/Inventory/IceAxe/BP_IceAxe_Inventory.BP_IceAxe_Inventory_C" => {
            Some("ピッケル")
        }
        "/Game/Blueprints/Inventory/Cleaver/BP_Cleaver_Inventory.BP_Cleaver_Inventory_C" => {
            Some("包丁")
        }
        "/Game/Blueprints/Inventory/Shovel/BP_Shovel_Inventory.BP_Shovel_Inventory_C" => {
            Some("シャベル")
        }
        "/Game/Blueprints/Inventory/Stick/BP_Stick_Inventory.BP_Stick_Inventory_C" => Some("木"),
        "/Game/Blueprints/Inventory/Rock/BP_Rock_Inventory.BP_Rock_Inventory_C" => Some("石"),
        "/Game/Blueprints/Inventory/Metals/BP_IronIngot_Inventory.BP_IronIngot_Inventory_C" => {
            Some("スクラップ")
        }
        "/Game/Blueprints/Inventory/Metals/BP_Nails_Inventory.BP_Nails_Inventory_C" => Some("釘"),
        "/Game/Blueprints/Inventory/Metals/BP_LeadIngot_Inventory.BP_LeadIngot_Inventory_C" => {
            Some("鉛")
        }
        "/Game/Blueprints/Inventory/Coal/BP_Coal_Inventory.BP_Coal_Inventory_C" => Some("石炭"),
        "/Game/Blueprints/Inventory/Flintlock/BP_Gunpowder_Inventory.BP_Gunpowder_Inventory_C" => {
            Some("火薬")
        }
        "/Game/Blueprints/Inventory/AnimalParts/BP_Sinew_Inventory.BP_Sinew_Inventory_C" => {
            Some("腱")
        }
        "/Game/Blueprints/Inventory/AnimalParts/BP_WolfPelt_Inventory.BP_WolfPelt_Inventory_C" => {
            Some("毛皮")
        }
        "/Game/Blueprints/Inventory/Tea/BP_Herbs_Inventory.BP_Herbs_Inventory_C" => {
            Some("ドライハーブ")
        }
        "/Game/Blueprints/Inventory/Syringe/BP_Syringe_Inventory.BP_Syringe_Inventory_C" => {
            Some("注射器")
        }
        "/Game/Blueprints/Inventory/Poison/BP_Antidote_Inventory.BP_Antidote_Inventory_C" => {
            Some("解毒剤")
        }
        "/Game/Blueprints/Inventory/Syringe/BP_Inventory_Laudanum.BP_Inventory_Laudanum_C" => {
            Some("アヘンチンキ")
        }
        "/Game/Blueprints/Inventory/Meat/BP_AnimalMeat_Inventory.BP_AnimalMeat_Inventory_C" => {
            Some("動物肉")
        }
        "/Game/Blueprints/Inventory/Meat/BP_CookedMeat_Inventory.BP_CookedMeat_Inventory_C" => {
            Some("焼肉")
        }
        "/Game/Blueprints/Inventory/Meat/BP_HumanMeat_Inventory.BP_HumanMeat_Inventory_C" => {
            Some("人肉")
        }
        "/Game/Blueprints/Inventory/Meat/BP_BoneClub_Inventory.BP_BoneClub_Inventory_C" => {
            Some("骨の棒")
        }
        "/Game/Blueprints/Inventory/AnimalParts/BP_Blubber_Inventory.BP_Blubber_Inventory_C" => {
            Some("脂")
        }
        "/Game/Blueprints/Inventory/Meat/BP_Stew_Inventory.BP_Stew_Inventory_C" => {
            Some("シチュー")
        }
        "/Game/Blueprints/Inventory/Tea/BP_Tea_Inventory.BP_Tea_Inventory_C" => Some("お茶"),
        "/Game/Blueprints/Inventory/Metals/BP_Whetstone_Inventory.BP_Whetstone_Inventory_C" => {
            Some("砥石")
        }
        "/Game/Blueprints/Inventory/BearTrap/BP_BearTrap_Inventory.BP_BearTrap_Inventory_C" => {
            Some("熊罠")
        }
        "/Game/Blueprints/Inventory/Spyglass/BP_Spyglass_Inventory.BP_Spyglass_Inventory_C" => {
            Some("望遠鏡")
        }
        "/Game/Blueprints/Inventory/Lantern/BP_Lantern_Inventory.BP_Lantern_Inventory_C" => {
            Some("ランタン")
        }
        "/Game/Blueprints/Inventory/Powderkeg/BP_CoalBarrel_Inventory.BP_CoalBarrel_Inventory_C" => {
            Some("石炭樽")
        }
        "/Game/Blueprints/Inventory/Powderkeg/BP_Powderkeg_Inventory.BP_Powderkeg_Inventory_C" => {
            Some("火薬樽")
        }
        "/Game/Blueprints/Inventory/Poison/BP_Poison_Inventory.BP_Poison_Inventory_C" => {
            Some("毒薬")
        }
        "/Game/Blueprints/Environment/Nitro/BP_Nitro_Inventory.BP_Nitro_Inventory_C" => {
            Some("ニトログリセリン")
        }
        "/Game/Blueprints/Inventory/LockPick/BP_SkeletonKey_Inventory.BP_SkeletonKey_Inventory_C" => {
            Some("骨の鍵")
        }
        "/Game/Blueprints/Inventory/LockPick/BP_CaptainsKey_Inventory.BP_CaptainsKey_Inventory_C" => {
            Some("船長室の鍵")
        }
        "/Game/Blueprints/Inventory/Armory/BP_Code_Inventory.BP_Code_Inventory_C" => {
            Some("武器庫の暗号")
        }
        "/Game/Blueprints/Inventory/Quest/BP_Quest_Inventory.BP_Quest_Inventory_C" => {
            Some("任務の巻物")
        }
        "/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C" => {
            Some("骨の短剣")
        }
        _ => None,
    }
}

#[must_use]
pub fn api_text(code: ApiCode) -> &'static str {
    match code {
        ApiCode::Success => "成功しました!",
        ApiCode::NetworkParamError => {
            "ネットワーク問題で引数を取得できませんでした。もう一度試してください!"
        }
        ApiCode::ServerExecutionError => {
            "サーバー側で問題が発生しました。試合開始後にもう一度試すか、サーバーの再起動・JSパッチをご確認ください!"
        }
        ApiCode::InvalidValueError => "数値エラー!",
        ApiCode::HighValueWarning => "送信成功! 数値が高すぎるとエラーの可能性があります!",
        ApiCode::BroadcastPartialError => "送信成功しましたが、自動ブロードキャストに失敗しました!",
    }
}

#[must_use]
pub fn log_category_text(code: LogCategoryCode) -> &'static str {
    match code {
        LogCategoryCode::Command => "コマンド",
        LogCategoryCode::Status => "ステータス",
        LogCategoryCode::Item => "アイテム",
        LogCategoryCode::Storage => "ストレージ",
        LogCategoryCode::Notice => "通知",
        LogCategoryCode::Broadcast => "全体通知",
        LogCategoryCode::Kick => "キック",
    }
}

#[must_use]
pub fn render_result_badge(code: ResultBadgeCode, suffix: &str) -> String {
    match code {
        ResultBadgeCode::Sent => "[送信済]".to_string(),
        ResultBadgeCode::PartialFailure => "[一部送信失敗]".to_string(),
        ResultBadgeCode::SendFailed => format!("[送信失敗{suffix}]"),
        ResultBadgeCode::Invalid => format!("[無効{suffix}]"),
    }
}

#[must_use]
pub fn detail_text(code: ConsoleLogDetailCode) -> &'static str {
    match code {
        ConsoleLogDetailCode::InvalidParams => "パラメータ不正",
        ConsoleLogDetailCode::InvalidNumberReason => "不正な数値",
        ConsoleLogDetailCode::InvalidCountReason => "不正な個数",
        ConsoleLogDetailCode::InvalidValueReason => "不正な値",
        ConsoleLogDetailCode::BoneDaggerAddedSuffix => " (+骨の短剣)",
        ConsoleLogDetailCode::StorageCapacityLabel => "容量 ->",
        ConsoleLogDetailCode::BroadcastAllTarget => "全員",
        ConsoleLogDetailCode::KickVerb => "をキック",
        ConsoleLogDetailCode::EndMatchExplorersWin => "探検家勝利",
        ConsoleLogDetailCode::EndMatchThrallsWin => "傀儡勝利",
    }
}

#[must_use]
pub fn quote_message(message: &str) -> String {
    format!("「{message}」")
}

#[must_use]
pub fn render_broadcast(event: &BroadcastEvent<'_>) -> String {
    match event {
        BroadcastEvent::Summon {
            role_key,
            target_key,
        } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            let target: &str = actor_display_name(target_key).unwrap_or(target_key);
            format!("{role} が{target}を召喚しました")
        }
        BroadcastEvent::UnsetThrall { role_key } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} が船員に戻りました")
        }
        BroadcastEvent::SetThrall { role_key } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} が闇の傀儡に悪墜ちしました")
        }
        BroadcastEvent::SpellCharge { role_key } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} がスペールチャージしました")
        }
        BroadcastEvent::StopBlizzard { role_key } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} が暴風雪を停止しました")
        }
        BroadcastEvent::StartBlizzard { role_key } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} が暴風雪を呼びました")
        }
        BroadcastEvent::ResetTimeScale { role_key } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} が世界を元に戻しました")
        }
        BroadcastEvent::AccelerateTimeScale { role_key } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} が世界を加速しました")
        }
        BroadcastEvent::DisableGodMode { role_key } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} が無敵状態を解除しました")
        }
        BroadcastEvent::EnableGodMode { role_key } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} が無敵状態になりました")
        }
        BroadcastEvent::ModifiedStats { role_key } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} が自分のステータスを操作しました")
        }
        BroadcastEvent::AcquiredItem {
            role_key,
            item_key,
            count,
        } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            let item: &str = actor_display_name(item_key).unwrap_or(item_key);
            format!("{role} が {item} {count} を獲得しました")
        }
        BroadcastEvent::StorageLimitChanged { role_key, limit } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} のストレージ容量が {limit} になりました")
        }
        BroadcastEvent::Kicked { role_key } => {
            let role: &str = player_role_name(role_key).unwrap_or(role_key);
            format!("{role} がキックされました")
        }
    }
}
