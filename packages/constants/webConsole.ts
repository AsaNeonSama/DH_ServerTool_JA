import type { PlayerRoleName } from "../types";

export const PLAYER_NAME_LIST: readonly PlayerRoleName[] = [
  "Captain",
  "Chaplain",
  "Cook",
  "Doctor",
  "Engineer",
  "Hunter",
  "Marine",
  "Navigator",
];

export const PLAYER_NAME_DICT: Readonly<Record<string, string>> = {
  Captain: "船長",
  Chaplain: "牧師",
  Cook: "料理人",
  Doctor: "医者",
  Engineer: "エンジニア",
  Hunter: "猟師",
  Marine: "海兵",
  Navigator: "航海士",
};

export const ACTOR_COMMENT_DICT: Readonly<Record<string, string>> = {
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
};

export const BONE_DAGGER_ITEM: string =
  "/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C";

export const STATUS_CODE: Readonly<
  Record<
    "success" | "badRequest" | "serverError" | "invalidValue" | "partialSuccess",
    number
  >
> = {
  success: 200,
  badRequest: 400,
  serverError: 504,
  invalidValue: 416,
  partialSuccess: 207,
};

export const CONSOLE_MESSAGES: Readonly<
  Record<
    | "success"
    | "networkParamError"
    | "serverExecutionError"
    | "invalidValueError"
    | "highValueWarning"
    | "broadcastPartialError",
    string
  >
> = {
  success: "成功しました!",
  networkParamError:
    "ネットワーク問題で引数を取得できませんでした。もう一度試してください!",
  serverExecutionError:
    "サーバー側で問題が発生しました。試合開始後にもう一度試すか、サーバーの再起動・JSパッチをご確認ください!",
  invalidValueError: "数値エラー!",
  highValueWarning: "送信成功! 数値が高すぎるとエラーの可能性があります!",
  broadcastPartialError: "送信成功しましたが、自動ブロードキャストに失敗しました!",
};

export const COOKIE_NAME: string = "dh_console_auth";
export const COOKIE_MAX_AGE_SECONDS: number = 315_360_000;
export const DEFAULT_TCP_TIMEOUT_MS: number = 1500;
export const HIGH_STAT_WARNING_THRESHOLD: number = 500;
