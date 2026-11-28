pub const EVENT_CONSOLE_LOG: &str = "dh://console-log";
pub const EVENT_SERVER_LOG: &str = "dh://server-log";
pub const EVENT_SERVER_STATE: &str = "dh://server-state";
pub const EMBEDDED_LICENSE_TEXT: &str = include_str!("../../../../LICENSE");

pub const MAX_CONSOLE_LOG_ENTRIES: usize = 300;
pub const MAX_SERVER_LOG_ENTRIES: usize = 400;

pub const PRACTICAL_MOD_MAX_NUMBER: u32 = 13;

pub const MIN_MAX_PLAYERS: u32 = 1;
pub const MAX_MAX_PLAYERS: u32 = 8;
pub const MIN_DAYS_BEFORE_BLIZZARD: u32 = 2;
pub const MAX_DAYS_BEFORE_BLIZZARD: u32 = 7;
pub const MIN_DAY_MINUTES: u32 = 5;
pub const MAX_DAY_MINUTES: u32 = 16;
pub const MIN_THRALLS: u32 = 0;
pub const MAX_THRALLS: u32 = 8;
pub const MIN_MULTIPLIER_QUARTER: f64 = 0.25;
pub const MAX_MULTIPLIER_THREE: f64 = 3.0;
pub const MIN_COAL_BURN_RATE: f64 = 0.1;
pub const MAX_COAL_BURN_RATE: f64 = 2.0;

pub const CONTENT_MOD_ID_TOTEM_SPELLS: &str = "content_totem_spells";
pub const CONTENT_MOD_ID_UE_VOICE: &str = "content_ue_voice";
pub const REQ_BADGE_PAK: &str = "[PAK必須]";
pub const REQ_DETAIL_TOTEM_SPELLS: &str = "DH_TotemSpells_Client_p.{pak,sig}";
pub const REQ_BADGE_DLL: &str = "[DLL必須]";
pub const REQ_DETAIL_UE_VOICE: &str = "dh_voice_client.dll";

pub const CANNIBAL_RANDOM_KEY: &str = "CANNIBAL_RANDOM";
pub const CANNIBAL_VARIANTS: [&str; 3] = [
    "BP_Cannibal_C",
    "BP_Cannibal_Crawler_C",
    "BP_Cannibal_Shambler_C",
];

pub const CONSOLE_COMMAND_TYPES: [(&str, &str); 8] = [
    ("summon", "召喚"),
    ("isThrall", "傀儡設置"),
    ("spellCharge", "スペールチャージ"),
    ("blizzardIntensity", "暴風雪設定"),
    ("slomo", "ゲーム加速"),
    ("god", "無敵設置"),
    ("endMatch", "ゲーム終了"),
    ("raw", "カスタム命令 (直接入力)"),
];

pub const CONSOLE_SUMMON_KEYS: [&str; 11] = [
    "BP_Prey_Rat_C",
    "BP_Prey_Rabbit_C",
    "BP_Prey_Seal_C",
    "BP_Predator_Wolf_C",
    "BP_Predator_Bear_C",
    CANNIBAL_RANDOM_KEY,
    "BP_HumanCharacter_C",
    "BP_HumanCharacter_Thrall_C",
    "BP_Totem_C",
    "BP_ExplosionActor_C",
    "BP_Explosion_Nitro_C",
];

pub const CONSOLE_SPELL_CHARGES: [(&str, &str); 3] = [
    ("0.37", "レベル１"),
    ("0.65", "レベル２"),
    ("1.00", "レベル３"),
];

pub const CONSOLE_BLIZZARD_INTENSITIES: [(&str, &str); 5] = [
    ("0.15", "軽い暴風雪"),
    ("0.35", "中程の暴風雪"),
    ("0.67", "重い暴風雪"),
    ("1.00", "冬将軍の召喚"),
    ("0.00", "暴風雪終了"),
];

pub const CONSOLE_SLOMO_OPTIONS: [(&str, &str); 5] = [
    ("2", "二倍加速"),
    ("5", "五倍加速"),
    ("10", "十倍加速"),
    ("50", "メイド・イン・ヘブン"),
    ("1", "元に戻す"),
];

pub const CONSOLE_STATUS_SPECS: [(&str, &str, bool); 13] = [
    ("bSpiritWalking", "幽体状態", true),
    ("bPoisoned", "中毒状態", true),
    ("CurrentHealth", "HP値", false),
    ("MaximumStamina", "最大スタミナ", false),
    ("MinStamina", "最小スタミナ", false),
    ("MaximumHealth", "最大HP値", false),
    ("MinHealth", "最小HP値", false),
    ("WarmthRegenRate", "温暖値回復速率", false),
    ("HungerRegenRate", "食糧度回復速率", false),
    ("HealthBoostRegenRate", "HP値回復速率", false),
    ("WarmthBoostRemaining", "温暖値持続回復時間", false),
    ("HungerBoostRemaining", "食糧度持続回復時間", false),
    ("HealthBoostRemaining", "HP値持続回復時間", false),
];

pub const CONSOLE_ITEM_PATHS: [&str; 44] = [
    "/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Inventory.BP_Flintlock_Inventory_C",
    "/Game/Blueprints/Inventory/Musket/BP_Musket_Inventory.BP_Musket_Inventory_C",
    "/Game/Blueprints/Inventory/Bow/BP_Bow_Inventory.BP_Bow_Inventory_C",
    "/Game/Blueprints/Inventory/Flint/BP_GunParts_Inventory.BP_GunParts_Inventory_C",
    "/Game/Blueprints/Inventory/Flintlock/BP_Flintlock_Ammo_Inventory.BP_Flintlock_Ammo_Inventory_C",
    "/Game/Blueprints/Inventory/Bow/BP_Arrows_Inventory.BP_Arrows_Inventory_C",
    "/Game/Blueprints/Inventory/Sword/BP_Sword_Inventory.BP_Sword_Inventory_C",
    "/Game/Blueprints/Inventory/WoodAxe/BP_WoodAxe_Inventory.BP_WoodAxe_Inventory_C",
    "/Game/Blueprints/Inventory/IceAxe/BP_IceAxe_Inventory.BP_IceAxe_Inventory_C",
    "/Game/Blueprints/Inventory/Cleaver/BP_Cleaver_Inventory.BP_Cleaver_Inventory_C",
    "/Game/Blueprints/Inventory/Shovel/BP_Shovel_Inventory.BP_Shovel_Inventory_C",
    "/Game/Blueprints/Inventory/Stick/BP_Stick_Inventory.BP_Stick_Inventory_C",
    "/Game/Blueprints/Inventory/Rock/BP_Rock_Inventory.BP_Rock_Inventory_C",
    "/Game/Blueprints/Inventory/Metals/BP_IronIngot_Inventory.BP_IronIngot_Inventory_C",
    "/Game/Blueprints/Inventory/Metals/BP_Nails_Inventory.BP_Nails_Inventory_C",
    "/Game/Blueprints/Inventory/Metals/BP_LeadIngot_Inventory.BP_LeadIngot_Inventory_C",
    "/Game/Blueprints/Inventory/Coal/BP_Coal_Inventory.BP_Coal_Inventory_C",
    "/Game/Blueprints/Inventory/Flintlock/BP_Gunpowder_Inventory.BP_Gunpowder_Inventory_C",
    "/Game/Blueprints/Inventory/AnimalParts/BP_Sinew_Inventory.BP_Sinew_Inventory_C",
    "/Game/Blueprints/Inventory/AnimalParts/BP_WolfPelt_Inventory.BP_WolfPelt_Inventory_C",
    "/Game/Blueprints/Inventory/Tea/BP_Herbs_Inventory.BP_Herbs_Inventory_C",
    "/Game/Blueprints/Inventory/Syringe/BP_Syringe_Inventory.BP_Syringe_Inventory_C",
    "/Game/Blueprints/Inventory/Poison/BP_Antidote_Inventory.BP_Antidote_Inventory_C",
    "/Game/Blueprints/Inventory/Syringe/BP_Inventory_Laudanum.BP_Inventory_Laudanum_C",
    "/Game/Blueprints/Inventory/Meat/BP_AnimalMeat_Inventory.BP_AnimalMeat_Inventory_C",
    "/Game/Blueprints/Inventory/Meat/BP_CookedMeat_Inventory.BP_CookedMeat_Inventory_C",
    "/Game/Blueprints/Inventory/Meat/BP_HumanMeat_Inventory.BP_HumanMeat_Inventory_C",
    "/Game/Blueprints/Inventory/Meat/BP_BoneClub_Inventory.BP_BoneClub_Inventory_C",
    "/Game/Blueprints/Inventory/AnimalParts/BP_Blubber_Inventory.BP_Blubber_Inventory_C",
    "/Game/Blueprints/Inventory/Meat/BP_Stew_Inventory.BP_Stew_Inventory_C",
    "/Game/Blueprints/Inventory/Tea/BP_Tea_Inventory.BP_Tea_Inventory_C",
    "/Game/Blueprints/Inventory/Metals/BP_Whetstone_Inventory.BP_Whetstone_Inventory_C",
    "/Game/Blueprints/Inventory/BearTrap/BP_BearTrap_Inventory.BP_BearTrap_Inventory_C",
    "/Game/Blueprints/Inventory/Spyglass/BP_Spyglass_Inventory.BP_Spyglass_Inventory_C",
    "/Game/Blueprints/Inventory/Lantern/BP_Lantern_Inventory.BP_Lantern_Inventory_C",
    "/Game/Blueprints/Inventory/Powderkeg/BP_CoalBarrel_Inventory.BP_CoalBarrel_Inventory_C",
    "/Game/Blueprints/Inventory/Powderkeg/BP_Powderkeg_Inventory.BP_Powderkeg_Inventory_C",
    "/Game/Blueprints/Inventory/Poison/BP_Poison_Inventory.BP_Poison_Inventory_C",
    "/Game/Blueprints/Environment/Nitro/BP_Nitro_Inventory.BP_Nitro_Inventory_C",
    "/Game/Blueprints/Inventory/LockPick/BP_SkeletonKey_Inventory.BP_SkeletonKey_Inventory_C",
    "/Game/Blueprints/Inventory/LockPick/BP_CaptainsKey_Inventory.BP_CaptainsKey_Inventory_C",
    "/Game/Blueprints/Inventory/Armory/BP_Code_Inventory.BP_Code_Inventory_C",
    "/Game/Blueprints/Inventory/Quest/BP_Quest_Inventory.BP_Quest_Inventory_C",
    "/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C",
];

pub const CONSOLE_ITEM_COUNTS: [i64; 5] = [1, 3, 5, 10, 20];

pub const CONSOLE_STORAGE_OPTIONS: [(&str, &str); 3] = [
    ("10", "バックパックの量"),
    ("20", "バックパックの量の二倍"),
    ("30", "バックパックの量の三倍"),
];

pub const GUI_APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_LICENSE_URL: &str = "https://github.com/AsaNeonSama/DH_Tool_JA/blob/main/LICENSE";
