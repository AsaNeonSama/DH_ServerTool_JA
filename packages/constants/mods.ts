// Default mods (always injected without asking)
import consoleWin64Js from "../../assets/mods/default/console_win64.js" with { type: "text" };
import deathOfflineAnnouncementJs from "../../assets/mods/default/death_offline_announcement_win64.js" with { type: "text" };
import networkFixJs from "../../assets/mods/default/network_fix_win64.js" with { type: "text" };
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
import gunAndBowJs from "../../assets/mods/single/gun_and_bow_win64.js" with { type: "text" };
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
import { MOD_MESSAGES } from "../messages";
import type { ModMenuOption, PatchScript } from "../types";

export const CONSOLE_PATCH_ID: string = "default:console_win64";
export const CONSOLE_PATCH_PORT_TOKEN: string = "__CONSOLE_PATCH_PORT__";

export const DEFAULT_PATCH_SCRIPTS: readonly PatchScript[] = [
  {
    id: CONSOLE_PATCH_ID,
    fileName: "console_win64.js",
    label: MOD_MESSAGES.defaults.console,
    group: "default",
    source: consoleWin64Js,
  },
  {
    id: "default:spell_charge_fix_win64",
    fileName: "spell_charge_fix_win64.js",
    label: MOD_MESSAGES.defaults.spellChargeFix,
    group: "default",
    source: spellChargeFixJs,
  },
  {
    id: "default:death_offline_announcement_win64",
    fileName: "death_offline_announcement_win64.js",
    label: MOD_MESSAGES.defaults.deathOfflineAnnouncement,
    group: "default",
    source: deathOfflineAnnouncementJs,
  },
  {
    id: "default:network_fix_win64",
    fileName: "network_fix_win64.js",
    label: MOD_MESSAGES.defaults.networkFix,
    group: "default",
    source: networkFixJs,
  },
];

export const SELECTABLE_MOD_OPTIONS: readonly ModMenuOption[] = [
  // Practical / standard competitive & QoL mods first
  {
    id: "rand_role",
    number: 1,
    label: MOD_MESSAGES.selectable.randRole,
    kind: "single",
    scripts: [
      {
        id: "single:rand_role_win64",
        fileName: "rand_role_win64.js",
        label: MOD_MESSAGES.selectable.randRole,
        group: "single",
        source: randRoleJs,
      },
    ],
  },
  {
    id: "spell_cast_start",
    number: 2,
    label: MOD_MESSAGES.selectable.spellCastStart,
    kind: "single",
    scripts: [
      {
        id: "single:spell_cast_start_win64",
        fileName: "spell_cast_start_win64.js",
        label: MOD_MESSAGES.selectable.spellCastStart,
        group: "single",
        source: spellCastStartJs,
      },
    ],
  },
  {
    id: "resources_spawner_departure",
    number: 3,
    label: MOD_MESSAGES.selectable.resourcesSpawnerDeparture,
    kind: "single",
    conflictsWith: ["sightseeing_departure"],
    scripts: [
      {
        id: "single:resources_spawner_Departure_win64",
        fileName: "resources_spawner_Departure_win64.js",
        label: MOD_MESSAGES.selectable.resourcesSpawnerDeparture,
        group: "single",
        source: resourcesSpawnerDepartureJs,
      },
    ],
  },
  {
    id: "resources_spawner_expanse",
    number: 4,
    label: MOD_MESSAGES.selectable.resourcesSpawnerExpanse,
    kind: "single",
    conflictsWith: ["sightseeing_expanse"],
    scripts: [
      {
        id: "single:resources_spawner_Expanse_win64",
        fileName: "resources_spawner_Expanse_win64.js",
        label: MOD_MESSAGES.selectable.resourcesSpawnerExpanse,
        group: "single",
        source: resourcesSpawnerExpanseJs,
      },
    ],
  },
  {
    id: "spell_custom",
    number: 5,
    label: MOD_MESSAGES.selectable.spellCustom,
    kind: "single",
    scripts: [
      {
        id: "single:spell_custom_win64",
        fileName: "spell_custom_win64.js",
        label: MOD_MESSAGES.selectable.spellCustom,
        group: "single",
        source: spellCustomJs,
      },
    ],
  },
  {
    id: "madmate_normal_nolimit",
    number: 6,
    label: MOD_MESSAGES.selectable.madmateNormalNolimit,
    kind: "single",
    scripts: [
      {
        id: "single:madmate_normal_nolimit",
        fileName: "madmate_normal_nolimit.js",
        label: MOD_MESSAGES.selectable.madmateNormalNolimit,
        group: "single",
        source: madmateNormalNolimitJs,
      },
    ],
  },
  {
    id: "bone_dagger_mode",
    number: 7,
    label: MOD_MESSAGES.selectable.boneDaggerMode,
    kind: "mode",
    scripts: [
      {
        id: "bone_dagger:bone_dagger_win64",
        fileName: "bone_dagger_win64.js",
        label: MOD_MESSAGES.selectable.boneDaggerScripts.main,
        group: "bone_dagger",
        source: boneDaggerJs,
      },
      {
        id: "bone_dagger:custom_droptable_bone_dagger_win64",
        fileName: "custom_droptable_bone_dagger_win64.js",
        label: MOD_MESSAGES.selectable.boneDaggerScripts.dropTable,
        group: "bone_dagger",
        source: customDroptableBoneDaggerJs,
      },
      {
        id: "bone_dagger:set_inventory_win64",
        fileName: "set_inventory_win64.js",
        label: MOD_MESSAGES.selectable.boneDaggerScripts.inventory,
        group: "bone_dagger",
        source: setInventoryJs,
      },
    ],
  },
  {
    id: "quest_system",
    number: 8,
    label: MOD_MESSAGES.selectable.questSystem,
    kind: "single",
    scripts: [
      {
        id: "single:quest_system_win64",
        fileName: "quest_system_win64.js",
        label: MOD_MESSAGES.selectable.questSystem,
        group: "single",
        source: questSystemJs,
      },
    ],
  },
  {
    id: "add_storage",
    number: 9,
    label: MOD_MESSAGES.selectable.addStorage,
    kind: "single",
    scripts: [
      {
        id: "single:add_storage_win64",
        fileName: "add_storage_win64.js",
        label: MOD_MESSAGES.selectable.addStorage,
        group: "single",
        source: addStorageJs,
      },
    ],
  },
  {
    id: "whetstone_any_item",
    number: 10,
    label: MOD_MESSAGES.selectable.whetstoneAnyItem,
    kind: "single",
    scripts: [
      {
        id: "single:whetstone_any_item_win64",
        fileName: "whetstone_any_item_win64.js",
        label: MOD_MESSAGES.selectable.whetstoneAnyItem,
        group: "single",
        source: whetstoneAnyItemJs,
      },
    ],
  },
  {
    id: "arrow_full_recovery",
    number: 11,
    label: MOD_MESSAGES.selectable.arrowFullRecovery,
    kind: "single",
    scripts: [
      {
        id: "single:arrow_full_recovery_win64",
        fileName: "arrow_full_recovery_win64.js",
        label: MOD_MESSAGES.selectable.arrowFullRecovery,
        group: "single",
        source: arrowFullRecoveryJs,
      },
    ],
  },
  {
    id: "blizzard_armory_open",
    number: 12,
    label: MOD_MESSAGES.selectable.blizzardArmoryOpen,
    kind: "single",
    conflictsWith: ["start_armory_open"],
    scripts: [
      {
        id: "single:blizzard_armory_open_win64",
        fileName: "blizzard_armory_open_win64.js",
        label: MOD_MESSAGES.selectable.blizzardArmoryOpen,
        group: "single",
        source: blizzardArmoryOpenJs,
      },
    ],
  },
  {
    id: "gun_and_bow",
    number: 13,
    label: MOD_MESSAGES.selectable.gunAndBow,
    kind: "single",
    scripts: [
      {
        id: "single:gun_and_bow_win64",
        fileName: "gun_and_bow_win64.js",
        label: MOD_MESSAGES.selectable.gunAndBow,
        group: "single",
        source: gunAndBowJs,
      },
    ],
  },
  // Fun / variant / party modes last
  {
    id: "start_armory_open",
    number: 14,
    label: MOD_MESSAGES.selectable.startArmoryOpen,
    kind: "single",
    conflictsWith: ["blizzard_armory_open"],
    scripts: [
      {
        id: "single:start_armory_open_win64",
        fileName: "start_armory_open_win64.js",
        label: MOD_MESSAGES.selectable.startArmoryOpen,
        group: "single",
        source: startArmoryOpenJs,
      },
    ],
  },
  {
    id: "vomit_modify",
    number: 15,
    label: MOD_MESSAGES.selectable.vomitModify,
    kind: "single",
    scripts: [
      {
        id: "single:vomit_modify_win64",
        fileName: "vomit_modify_win64.js",
        label: MOD_MESSAGES.selectable.vomitModify,
        group: "single",
        source: vomitModifyJs,
      },
    ],
  },
  {
    id: "north_ghost",
    number: 16,
    label: MOD_MESSAGES.selectable.northGhost,
    kind: "single",
    scripts: [
      {
        id: "single:north_ghost_win64",
        fileName: "north_ghost_win64.js",
        label: MOD_MESSAGES.selectable.northGhost,
        group: "single",
        source: northGhostJs,
      },
    ],
  },
  {
    id: "grenade",
    number: 17,
    label: MOD_MESSAGES.selectable.grenade,
    kind: "single",
    scripts: [
      {
        id: "single:grenade_win64",
        fileName: "grenade_win64.js",
        label: MOD_MESSAGES.selectable.grenade,
        group: "single",
        source: grenadeJs,
      },
    ],
  },
  {
    id: "moon_jump",
    number: 18,
    label: MOD_MESSAGES.selectable.moonJump,
    kind: "single",
    scripts: [
      {
        id: "single:moon_jump_win64",
        fileName: "moon_jump_win64.js",
        label: MOD_MESSAGES.selectable.moonJump,
        group: "single",
        source: moonJumpJs,
      },
    ],
  },
  {
    id: "spell_no_cd",
    number: 19,
    label: MOD_MESSAGES.selectable.spellNoCd,
    kind: "single",
    scripts: [
      {
        id: "single:spell_no_cd_win64",
        fileName: "spell_no_cd_win64.js",
        label: MOD_MESSAGES.selectable.spellNoCd,
        group: "single",
        source: spellNoCdJs,
      },
    ],
  },
  {
    id: "no_death",
    number: 20,
    label: MOD_MESSAGES.selectable.noDeath,
    kind: "single",
    scripts: [
      {
        id: "single:no_death_win64",
        fileName: "no_death_win64.js",
        label: MOD_MESSAGES.selectable.noDeath,
        group: "single",
        source: noDeathJs,
      },
    ],
  },
  {
    id: "sightseeing_departure",
    number: 21,
    label: MOD_MESSAGES.selectable.sightseeingDeparture,
    kind: "single",
    conflictsWith: ["resources_spawner_departure", "sightseeing_expanse"],
    scripts: [
      {
        id: "single:sightseeing_departure_win64",
        fileName: "sightseeing_departure_win64.js",
        label: MOD_MESSAGES.selectable.sightseeingDeparture,
        group: "single",
        source: sightseeingDepartureJs,
      },
    ],
  },
  {
    id: "sightseeing_expanse",
    number: 22,
    label: MOD_MESSAGES.selectable.sightseeingExpanse,
    kind: "single",
    conflictsWith: ["resources_spawner_expanse", "sightseeing_departure"],
    scripts: [
      {
        id: "single:sightseeing_expanse_win64",
        fileName: "sightseeing_expanse_win64.js",
        label: MOD_MESSAGES.selectable.sightseeingExpanse,
        group: "single",
        source: sightseeingExpanseJs,
      },
    ],
  },
];
