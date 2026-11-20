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
import grenadeJs from "../../assets/mods/single/grenade_win64.js" with { type: "text" };
import madmateNormalNolimitJs from "../../assets/mods/single/madmate_normal_nolimit.js" with { type: "text" };
import moonJumpJs from "../../assets/mods/single/moon_jump_win64.js" with { type: "text" };
import noDeathJs from "../../assets/mods/single/no_death_win64.js" with { type: "text" };
import northGhostJs from "../../assets/mods/single/north_ghost_win64.js" with { type: "text" };
import randRoleJs from "../../assets/mods/single/rand_role_win64.js" with { type: "text" };
import resourcesSpawnerDepartureJs from "../../assets/mods/single/resources_spawner_Departure_win64.js" with { type: "text" };
import resourcesSpawnerExpanseJs from "../../assets/mods/single/resources_spawner_Expanse_win64.js" with { type: "text" };
import spellCastStartJs from "../../assets/mods/single/spell_cast_start_win64.js" with { type: "text" };
import spellCustomJs from "../../assets/mods/single/spell_custom_win64.js" with { type: "text" };
import spellNoCdJs from "../../assets/mods/single/spell_no_cd_win64.js" with { type: "text" };
import startArmoryOpenJs from "../../assets/mods/single/start_armory_open_win64.js" with { type: "text" };
import vomitModifyJs from "../../assets/mods/single/vomit_modify_win64.js" with { type: "text" };
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
  {
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
    number: 2,
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
    number: 3,
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
    number: 4,
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
    number: 5,
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
    number: 6,
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
    number: 7,
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
    number: 8,
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
    number: 9,
    label: "マップ補強(山頂)",
    kind: "single",
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
    number: 10,
    label: "マップ補強(広漠)",
    kind: "single",
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
    number: 11,
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
    number: 12,
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
    number: 13,
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
    number: 14,
    label: "開幕マスケット",
    kind: "single",
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
    number: 15,
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
];
