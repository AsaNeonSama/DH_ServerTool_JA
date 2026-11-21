// Default mods (always injected without asking)
import consoleWin64Js from "../../assets/mods/default/console_win64.js" with { type: "text" };
import deathOfflineAnnouncementJs from "../../assets/mods/default/death_offline_announcement_win64.js" with { type: "text" };
import spellChargeFixJs from "../../assets/mods/default/spell_charge_fix_win64.js" with { type: "text" };

// Bone dagger mode (injected together as a single mode choice)
import boneDaggerJs from "../../assets/mods/bone_dagger/bone_dagger_win64.js" with { type: "text" };
import customDroptableBoneDaggerJs from "../../assets/mods/bone_dagger/custom_droptable_bone_dagger_win64.js" with { type: "text" };
import setInventoryJs from "../../assets/mods/bone_dagger/set_inventory_win64.js" with { type: "text" };

// Single mods (each selectable individually)
import addStorageJs from "../../assets/mods/single/add_storage_win64.js" with { type: "text" };
import arrowFullRecoveryJs from "../../assets/mods/single/arrow_full_recovery_win64.js" with { type: "text" };
import blizzardArmoryOpenJs from "../../assets/mods/single/blizzard_armory_open_win64.js" with { type: "text" };
import grenadeJs from "../../assets/mods/single/grenade_win64.js" with { type: "text" };
import madmateNormalNolimitJs from "../../assets/mods/single/madmate_normal_nolimit.js" with { type: "text" };
import moonJumpJs from "../../assets/mods/single/moon_jump_win64.js" with { type: "text" };
import noDeathJs from "../../assets/mods/single/no_death_win64.js" with { type: "text" };
import northGhostJs from "../../assets/mods/single/north_ghost_win64.js" with { type: "text" };
import questSystemJs from "../../assets/mods/single/quest_system_win64.js" with { type: "text" };
import randRoleJs from "../../assets/mods/single/rand_role_win64.js" with { type: "text" };
import resourcesSpawnerDepartureJs from "../../assets/mods/single/resources_spawner_Departure_win64.js" with { type: "text" };
import resourcesSpawnerExpanseJs from "../../assets/mods/single/resources_spawner_Expanse_win64.js" with { type: "text" };
import sightseeingDepartureJs from "../../assets/mods/single/sightseeing_departure_win64.js" with { type: "text" };
import sightseeingExpanseJs from "../../assets/mods/single/sightseeing_expanse_win64.js" with { type: "text" };
import spellCastStartJs from "../../assets/mods/single/spell_cast_start_win64.js" with { type: "text" };
import spellCustomJs from "../../assets/mods/single/spell_custom_win64.js" with { type: "text" };
import spellNoCdJs from "../../assets/mods/single/spell_no_cd_win64.js" with { type: "text" };
import startArmoryOpenJs from "../../assets/mods/single/start_armory_open_win64.js" with { type: "text" };
import vomitModifyJs from "../../assets/mods/single/vomit_modify_win64.js" with { type: "text" };
import whetstoneAnyItemJs from "../../assets/mods/single/whetstone_any_item_win64.js" with { type: "text" };
import type { ModMenuOption, PatchScript } from "../types";

export const CONSOLE_PATCH_ID: string = "default:console_win64";
export const CONSOLE_PATCH_PORT_TOKEN: string = "__CONSOLE_PATCH_PORT__";

export const DEFAULT_PATCH_SCRIPTS: readonly PatchScript[] = [
  {
    id: CONSOLE_PATCH_ID,
    fileName: "console_win64.js",
    label: "Webコンソール連携",
    group: "default",
    source: consoleWin64Js,
  },
  {
    id: "default:spell_charge_fix_win64",
    fileName: "spell_charge_fix_win64.js",
    label: "スペルチャージ修正",
    group: "default",
    source: spellChargeFixJs,
  },
  {
    id: "default:death_offline_announcement_win64",
    fileName: "death_offline_announcement_win64.js",
    label: "オフラインアナウンス",
    group: "default",
    source: deathOfflineAnnouncementJs,
  },
];

export const SELECTABLE_MOD_OPTIONS: readonly ModMenuOption[] = [
  // Practical / standard competitive & QoL mods first
  {
    id: "rand_role",
    number: 1,
    label: "役職ランダム",
    kind: "single",
    scripts: [
      {
        id: "single:rand_role_win64",
        fileName: "rand_role_win64.js",
        label: "役職ランダム",
        group: "single",
        source: randRoleJs,
      },
    ],
  },
  {
    id: "spell_cast_start",
    number: 2,
    label: "開幕沈黙",
    kind: "single",
    scripts: [
      {
        id: "single:spell_cast_start_win64",
        fileName: "spell_cast_start_win64.js",
        label: "開幕沈黙",
        group: "single",
        source: spellCastStartJs,
      },
    ],
  },
  {
    id: "resources_spawner_departure",
    number: 3,
    label: "マップ補強(山頂)",
    kind: "single",
    conflictsWith: ["sightseeing_departure"],
    scripts: [
      {
        id: "single:resources_spawner_Departure_win64",
        fileName: "resources_spawner_Departure_win64.js",
        label: "マップ補強(山頂)",
        group: "single",
        source: resourcesSpawnerDepartureJs,
      },
    ],
  },
  {
    id: "resources_spawner_expanse",
    number: 4,
    label: "マップ補強(広漠)",
    kind: "single",
    conflictsWith: ["sightseeing_expanse"],
    scripts: [
      {
        id: "single:resources_spawner_Expanse_win64",
        fileName: "resources_spawner_Expanse_win64.js",
        label: "マップ補強(広漠)",
        group: "single",
        source: resourcesSpawnerExpanseJs,
      },
    ],
  },
  {
    id: "spell_custom",
    number: 5,
    label: "五サボタージュ",
    kind: "single",
    scripts: [
      {
        id: "single:spell_custom_win64",
        fileName: "spell_custom_win64.js",
        label: "五サボタージュ",
        group: "single",
        source: spellCustomJs,
      },
    ],
  },
  {
    id: "madmate_normal_nolimit",
    number: 6,
    label: "狂人モード",
    kind: "single",
    scripts: [
      {
        id: "single:madmate_normal_nolimit",
        fileName: "madmate_normal_nolimit.js",
        label: "狂人モード",
        group: "single",
        source: madmateNormalNolimitJs,
      },
    ],
  },
  {
    id: "bone_dagger_mode",
    number: 7,
    label: "骨の短剣モード",
    kind: "mode",
    scripts: [
      {
        id: "bone_dagger:bone_dagger_win64",
        fileName: "bone_dagger_win64.js",
        label: "骨の短剣モード (短剣本体)",
        group: "bone_dagger",
        source: boneDaggerJs,
      },
      {
        id: "bone_dagger:custom_droptable_bone_dagger_win64",
        fileName: "custom_droptable_bone_dagger_win64.js",
        label: "骨の短剣モード (ドロップテーブル)",
        group: "bone_dagger",
        source: customDroptableBoneDaggerJs,
      },
      {
        id: "bone_dagger:set_inventory_win64",
        fileName: "set_inventory_win64.js",
        label: "骨の短剣モード (初期装備設定)",
        group: "bone_dagger",
        source: setInventoryJs,
      },
    ],
  },
  {
    id: "quest_system",
    number: 8,
    label: "クエスト報酬",
    kind: "single",
    scripts: [
      {
        id: "single:quest_system_win64",
        fileName: "quest_system_win64.js",
        label: "クエスト報酬",
        group: "single",
        source: questSystemJs,
      },
    ],
  },
  {
    id: "add_storage",
    number: 9,
    label: "全員バックパック",
    kind: "single",
    scripts: [
      {
        id: "single:add_storage_win64",
        fileName: "add_storage_win64.js",
        label: "全員バックパック",
        group: "single",
        source: addStorageJs,
      },
    ],
  },
  {
    id: "whetstone_any_item",
    number: 10,
    label: "万能砥石",
    kind: "single",
    scripts: [
      {
        id: "single:whetstone_any_item_win64",
        fileName: "whetstone_any_item_win64.js",
        label: "万能砥石",
        group: "single",
        source: whetstoneAnyItemJs,
      },
    ],
  },
  {
    id: "arrow_full_recovery",
    number: 11,
    label: "矢回収100%",
    kind: "single",
    scripts: [
      {
        id: "single:arrow_full_recovery_win64",
        fileName: "arrow_full_recovery_win64.js",
        label: "矢回収100%",
        group: "single",
        source: arrowFullRecoveryJs,
      },
    ],
  },
  {
    id: "blizzard_armory_open",
    number: 12,
    label: "暴風雪マスケット",
    kind: "single",
    conflictsWith: ["start_armory_open"],
    scripts: [
      {
        id: "single:blizzard_armory_open_win64",
        fileName: "blizzard_armory_open_win64.js",
        label: "暴風雪マスケット",
        group: "single",
        source: blizzardArmoryOpenJs,
      },
    ],
  },
  // Fun / variant / party modes last
  {
    id: "start_armory_open",
    number: 13,
    label: "開幕マスケット",
    kind: "single",
    conflictsWith: ["blizzard_armory_open"],
    scripts: [
      {
        id: "single:start_armory_open_win64",
        fileName: "start_armory_open_win64.js",
        label: "開幕マスケット",
        group: "single",
        source: startArmoryOpenJs,
      },
    ],
  },
  {
    id: "vomit_modify",
    number: 14,
    label: "毒耐性",
    kind: "single",
    scripts: [
      {
        id: "single:vomit_modify_win64",
        fileName: "vomit_modify_win64.js",
        label: "毒耐性",
        group: "single",
        source: vomitModifyJs,
      },
    ],
  },
  {
    id: "north_ghost",
    number: 15,
    label: "ミステリーモード",
    kind: "single",
    scripts: [
      {
        id: "single:north_ghost_win64",
        fileName: "north_ghost_win64.js",
        label: "ミステリーモード",
        group: "single",
        source: northGhostJs,
      },
    ],
  },
  {
    id: "grenade",
    number: 16,
    label: "グレネード",
    kind: "single",
    scripts: [
      {
        id: "single:grenade_win64",
        fileName: "grenade_win64.js",
        label: "グレネード",
        group: "single",
        source: grenadeJs,
      },
    ],
  },
  {
    id: "moon_jump",
    number: 17,
    label: "八艘飛び",
    kind: "single",
    scripts: [
      {
        id: "single:moon_jump_win64",
        fileName: "moon_jump_win64.js",
        label: "八艘飛び",
        group: "single",
        source: moonJumpJs,
      },
    ],
  },
  {
    id: "spell_no_cd",
    number: 18,
    label: "スペルチャージ無限",
    kind: "single",
    scripts: [
      {
        id: "single:spell_no_cd_win64",
        fileName: "spell_no_cd_win64.js",
        label: "スペルチャージ無限",
        group: "single",
        source: spellNoCdJs,
      },
    ],
  },
  {
    id: "no_death",
    number: 19,
    label: "無限復活",
    kind: "single",
    scripts: [
      {
        id: "single:no_death_win64",
        fileName: "no_death_win64.js",
        label: "無限復活",
        group: "single",
        source: noDeathJs,
      },
    ],
  },
  {
    id: "sightseeing_departure",
    number: 20,
    label: "観光モード(山頂)",
    kind: "single",
    conflictsWith: ["resources_spawner_departure", "sightseeing_expanse"],
    scripts: [
      {
        id: "single:sightseeing_departure_win64",
        fileName: "sightseeing_departure_win64.js",
        label: "観光モード(山頂)",
        group: "single",
        source: sightseeingDepartureJs,
      },
    ],
  },
  {
    id: "sightseeing_expanse",
    number: 21,
    label: "観光モード(広漠)",
    kind: "single",
    conflictsWith: ["resources_spawner_expanse", "sightseeing_departure"],
    scripts: [
      {
        id: "single:sightseeing_expanse_win64",
        fileName: "sightseeing_expanse_win64.js",
        label: "観光モード(広漠)",
        group: "single",
        source: sightseeingExpanseJs,
      },
    ],
  },
];
