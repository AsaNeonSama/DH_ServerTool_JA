use std::collections::HashSet;

use crate::messages::ja::mods as mod_msg;

pub const CONSOLE_PATCH_ID: &str = "default:console_win64";
pub const CONSOLE_PATCH_PORT_TOKEN: &str = "__CONSOLE_PATCH_PORT__";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModGroup {
    Default,
    Single,
    BoneDagger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModKind {
    Single,
    Mode,
}

#[derive(Debug, Clone, Copy)]
pub struct PatchScript {
    pub id: &'static str,
    pub file_name: &'static str,
    pub label: &'static str,
    pub group: ModGroup,
    pub source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPatchScript {
    pub id: String,
    pub file_name: String,
    pub label: String,
    pub group: ModGroup,
    pub source: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ModMenuOption {
    pub id: &'static str,
    pub number: u32,
    pub label: &'static str,
    pub kind: ModKind,
    pub scripts: &'static [PatchScript],
    pub conflicts_with: &'static [&'static str],
}

// Default mods (always injected first in exact order without prompting)
pub const DEFAULT_PATCH_SCRIPTS: &[PatchScript] = &[
    PatchScript {
        id: CONSOLE_PATCH_ID,
        file_name: "コンソール連携.js",
        label: mod_msg::defaults::CONSOLE,
        group: ModGroup::Default,
        source: include_str!("../../../../assets/mods/default/コンソール連携.js"),
    },
    PatchScript {
        id: "default:spell_charge_fix_win64",
        file_name: "スペルチャージ修正.js",
        label: mod_msg::defaults::SPELL_CHARGE_FIX,
        group: ModGroup::Default,
        source: include_str!("../../../../assets/mods/default/スペルチャージ修正.js"),
    },
    PatchScript {
        id: "default:death_offline_announcement_win64",
        file_name: "死亡切断通知.js",
        label: mod_msg::defaults::DEATH_OFFLINE_ANNOUNCEMENT,
        group: ModGroup::Default,
        source: include_str!("../../../../assets/mods/default/死亡切断通知.js"),
    },
    PatchScript {
        id: "default:network_fix_win64",
        file_name: "接続保護_乱入防止.js",
        label: mod_msg::defaults::NETWORK_FIX,
        group: ModGroup::Default,
        source: include_str!("../../../../assets/mods/default/接続保護_乱入防止.js"),
    },
];

// Selectable MOD menu options (1..=22; practical/competitive 1..=13, fun/variant 14..=22)
pub const SELECTABLE_MOD_OPTIONS: &[ModMenuOption] = &[
    ModMenuOption {
        id: "rand_role",
        number: 1,
        label: mod_msg::selectable::RAND_ROLE,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:rand_role_win64",
            file_name: "役職ランダム.js",
            label: mod_msg::selectable::RAND_ROLE,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/役職ランダム.js"),
        }],
    },
    ModMenuOption {
        id: "spell_cast_start",
        number: 2,
        label: mod_msg::selectable::SPELL_CAST_START,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:spell_cast_start_win64",
            file_name: "開幕沈黙.js",
            label: mod_msg::selectable::SPELL_CAST_START,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/開幕沈黙.js"),
        }],
    },
    ModMenuOption {
        id: "resources_spawner_departure",
        number: 3,
        label: mod_msg::selectable::RESOURCES_SPAWNER_DEPARTURE,
        kind: ModKind::Single,
        conflicts_with: &["sightseeing_departure"],
        scripts: &[PatchScript {
            id: "single:resources_spawner_Departure_win64",
            file_name: "マップ補強_山頂.js",
            label: mod_msg::selectable::RESOURCES_SPAWNER_DEPARTURE,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/マップ補強_山頂.js"),
        }],
    },
    ModMenuOption {
        id: "resources_spawner_expanse",
        number: 4,
        label: mod_msg::selectable::RESOURCES_SPAWNER_EXPANSE,
        kind: ModKind::Single,
        conflicts_with: &["sightseeing_expanse"],
        scripts: &[PatchScript {
            id: "single:resources_spawner_Expanse_win64",
            file_name: "マップ補強_広漠.js",
            label: mod_msg::selectable::RESOURCES_SPAWNER_EXPANSE,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/マップ補強_広漠.js"),
        }],
    },
    ModMenuOption {
        id: "spell_custom",
        number: 5,
        label: mod_msg::selectable::SPELL_CUSTOM,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:spell_custom_win64",
            file_name: "五サボタージュ.js",
            label: mod_msg::selectable::SPELL_CUSTOM,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/五サボタージュ.js"),
        }],
    },
    ModMenuOption {
        id: "madmate_normal_nolimit",
        number: 6,
        label: mod_msg::selectable::MADMATE_NORMAL_NOLIMIT,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:madmate_normal_nolimit",
            file_name: "狂人モード.js",
            label: mod_msg::selectable::MADMATE_NORMAL_NOLIMIT,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/狂人モード.js"),
        }],
    },
    ModMenuOption {
        id: "bone_dagger_mode",
        number: 7,
        label: mod_msg::selectable::BONE_DAGGER_MODE,
        kind: ModKind::Mode,
        conflicts_with: &[],
        scripts: &[
            PatchScript {
                id: "bone_dagger:bone_dagger_win64",
                file_name: "骨の短剣_本体.js",
                label: mod_msg::selectable::bone_dagger_scripts::MAIN,
                group: ModGroup::BoneDagger,
                source: include_str!("../../../../assets/mods/bone_dagger/骨の短剣_本体.js"),
            },
            PatchScript {
                id: "bone_dagger:custom_droptable_bone_dagger_win64",
                file_name: "骨の短剣_ドロップ.js",
                label: mod_msg::selectable::bone_dagger_scripts::DROP_TABLE,
                group: ModGroup::BoneDagger,
                source: include_str!("../../../../assets/mods/bone_dagger/骨の短剣_ドロップ.js"),
            },
            PatchScript {
                id: "bone_dagger:set_inventory_win64",
                file_name: "骨の短剣_初期装備.js",
                label: mod_msg::selectable::bone_dagger_scripts::INVENTORY,
                group: ModGroup::BoneDagger,
                source: include_str!("../../../../assets/mods/bone_dagger/骨の短剣_初期装備.js"),
            },
        ],
    },
    ModMenuOption {
        id: "quest_system",
        number: 8,
        label: mod_msg::selectable::QUEST_SYSTEM,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:quest_system_win64",
            file_name: "クエスト報酬.js",
            label: mod_msg::selectable::QUEST_SYSTEM,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/クエスト報酬.js"),
        }],
    },
    ModMenuOption {
        id: "add_storage",
        number: 9,
        label: mod_msg::selectable::ADD_STORAGE,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:add_storage_win64",
            file_name: "全員バックパック.js",
            label: mod_msg::selectable::ADD_STORAGE,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/全員バックパック.js"),
        }],
    },
    ModMenuOption {
        id: "whetstone_any_item",
        number: 10,
        label: mod_msg::selectable::WHETSTONE_ANY_ITEM,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:whetstone_any_item_win64",
            file_name: "万能砥石.js",
            label: mod_msg::selectable::WHETSTONE_ANY_ITEM,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/万能砥石.js"),
        }],
    },
    ModMenuOption {
        id: "arrow_full_recovery",
        number: 11,
        label: mod_msg::selectable::ARROW_FULL_RECOVERY,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:arrow_full_recovery_win64",
            file_name: "矢回収100.js",
            label: mod_msg::selectable::ARROW_FULL_RECOVERY,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/矢回収100.js"),
        }],
    },
    ModMenuOption {
        id: "blizzard_armory_open",
        number: 12,
        label: mod_msg::selectable::BLIZZARD_ARMORY_OPEN,
        kind: ModKind::Single,
        conflicts_with: &["start_armory_open"],
        scripts: &[PatchScript {
            id: "single:blizzard_armory_open_win64",
            file_name: "暴風雪マスケット.js",
            label: mod_msg::selectable::BLIZZARD_ARMORY_OPEN,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/暴風雪マスケット.js"),
        }],
    },
    ModMenuOption {
        id: "gun_and_bow",
        number: 13,
        label: mod_msg::selectable::GUN_AND_BOW,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:gun_and_bow_win64",
            file_name: "銃_弓同時所持.js",
            label: mod_msg::selectable::GUN_AND_BOW,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/銃_弓同時所持.js"),
        }],
    },
    ModMenuOption {
        id: "start_armory_open",
        number: 14,
        label: mod_msg::selectable::START_ARMORY_OPEN,
        kind: ModKind::Single,
        conflicts_with: &["blizzard_armory_open"],
        scripts: &[PatchScript {
            id: "single:start_armory_open_win64",
            file_name: "開幕マスケット.js",
            label: mod_msg::selectable::START_ARMORY_OPEN,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/開幕マスケット.js"),
        }],
    },
    ModMenuOption {
        id: "vomit_modify",
        number: 15,
        label: mod_msg::selectable::VOMIT_MODIFY,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:vomit_modify_win64",
            file_name: "毒耐性.js",
            label: mod_msg::selectable::VOMIT_MODIFY,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/毒耐性.js"),
        }],
    },
    ModMenuOption {
        id: "north_ghost",
        number: 16,
        label: mod_msg::selectable::NORTH_GHOST,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:north_ghost_win64",
            file_name: "ミステリーモード.js",
            label: mod_msg::selectable::NORTH_GHOST,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/ミステリーモード.js"),
        }],
    },
    ModMenuOption {
        id: "grenade",
        number: 17,
        label: mod_msg::selectable::GRENADE,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:grenade_win64",
            file_name: "グレネード.js",
            label: mod_msg::selectable::GRENADE,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/グレネード.js"),
        }],
    },
    ModMenuOption {
        id: "moon_jump",
        number: 18,
        label: mod_msg::selectable::MOON_JUMP,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:moon_jump_win64",
            file_name: "八艘飛び.js",
            label: mod_msg::selectable::MOON_JUMP,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/八艘飛び.js"),
        }],
    },
    ModMenuOption {
        id: "spell_no_cd",
        number: 19,
        label: mod_msg::selectable::SPELL_NO_CD,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:spell_no_cd_win64",
            file_name: "スペルチャージ無限.js",
            label: mod_msg::selectable::SPELL_NO_CD,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/スペルチャージ無限.js"),
        }],
    },
    ModMenuOption {
        id: "no_death",
        number: 20,
        label: mod_msg::selectable::NO_DEATH,
        kind: ModKind::Single,
        conflicts_with: &[],
        scripts: &[PatchScript {
            id: "single:no_death_win64",
            file_name: "無限復活.js",
            label: mod_msg::selectable::NO_DEATH,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/無限復活.js"),
        }],
    },
    ModMenuOption {
        id: "sightseeing_departure",
        number: 21,
        label: mod_msg::selectable::SIGHTSEEING_DEPARTURE,
        kind: ModKind::Single,
        conflicts_with: &["resources_spawner_departure", "sightseeing_expanse"],
        scripts: &[PatchScript {
            id: "single:sightseeing_departure_win64",
            file_name: "観光モード_山頂.js",
            label: mod_msg::selectable::SIGHTSEEING_DEPARTURE,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/観光モード_山頂.js"),
        }],
    },
    ModMenuOption {
        id: "sightseeing_expanse",
        number: 22,
        label: mod_msg::selectable::SIGHTSEEING_EXPANSE,
        kind: ModKind::Single,
        conflicts_with: &["resources_spawner_expanse", "sightseeing_departure"],
        scripts: &[PatchScript {
            id: "single:sightseeing_expanse_win64",
            file_name: "観光モード_広漠.js",
            label: mod_msg::selectable::SIGHTSEEING_EXPANSE,
            group: ModGroup::Single,
            source: include_str!("../../../../assets/mods/single/観光モード_広漠.js"),
        }],
    },
];

#[must_use]
pub fn find_option_by_number(number: u32) -> Option<&'static ModMenuOption> {
    SELECTABLE_MOD_OPTIONS
        .iter()
        .find(|item: &&ModMenuOption| item.number == number)
}

#[must_use]
pub fn are_mod_options_conflicting(a: &ModMenuOption, b: &ModMenuOption) -> bool {
    if a.id == b.id {
        return false;
    }
    a.conflicts_with.contains(&b.id) || b.conflicts_with.contains(&a.id)
}

#[must_use]
pub fn find_conflicting_selected_options(
    candidate: &ModMenuOption,
    selected_numbers: &[u32],
) -> Vec<&'static ModMenuOption> {
    let mut conflicts: Vec<&'static ModMenuOption> = Vec::new();
    for &num in selected_numbers {
        if let Some(selected) = find_option_by_number(num) {
            if are_mod_options_conflicting(candidate, selected) {
                conflicts.push(selected);
            }
        }
    }
    conflicts
}

#[must_use]
pub fn get_mod_conflict_pairs() -> Vec<(&'static ModMenuOption, &'static ModMenuOption)> {
    let mut seen_pairs: HashSet<(&'static str, &'static str)> = HashSet::new();
    let mut pairs: Vec<(&'static ModMenuOption, &'static ModMenuOption)> = Vec::new();

    for (i, a) in SELECTABLE_MOD_OPTIONS.iter().enumerate() {
        for b in SELECTABLE_MOD_OPTIONS.iter().skip(i + 1) {
            if !are_mod_options_conflicting(a, b) {
                continue;
            }
            if seen_pairs.insert((a.id, b.id)) {
                pairs.push((a, b));
            }
        }
    }

    pairs
}

/// Resolves the ordered list of scripts to inject for a match:
/// 4 default scripts first (applying `__CONSOLE_PATCH_PORT__` substitution to the console bridge),
/// followed by all non-conflicting selected optional MODs (expanding Bone Dagger into 3 scripts).
#[must_use]
pub fn build_injection_plan(
    selected_option_numbers: &[u32],
    console_patch_port: u16,
) -> Vec<ResolvedPatchScript> {
    let mut seen: HashSet<&'static str> = HashSet::new();
    let mut accepted_options: Vec<&'static ModMenuOption> = Vec::new();
    let mut result: Vec<ResolvedPatchScript> = Vec::new();

    let port_str: String = console_patch_port.to_string();

    for patch in DEFAULT_PATCH_SCRIPTS {
        let stripped: &str = patch
            .source
            .strip_prefix('\u{feff}')
            .unwrap_or(patch.source);
        let resolved_source: String = if patch.id == CONSOLE_PATCH_ID {
            stripped.replace(CONSOLE_PATCH_PORT_TOKEN, &port_str)
        } else {
            stripped.to_string()
        };
        seen.insert(patch.id);
        result.push(ResolvedPatchScript {
            id: patch.id.to_string(),
            file_name: patch.file_name.to_string(),
            label: patch.label.to_string(),
            group: patch.group,
            source: resolved_source,
        });
    }

    for &num in selected_option_numbers {
        let Some(opt): Option<&'static ModMenuOption> = find_option_by_number(num) else {
            continue;
        };
        let has_conflict: bool = accepted_options
            .iter()
            .any(|existing: &&ModMenuOption| are_mod_options_conflicting(existing, opt));
        if has_conflict {
            continue;
        }
        accepted_options.push(opt);
        for patch in opt.scripts {
            if !seen.insert(patch.id) {
                continue;
            }
            let stripped: &str = patch
                .source
                .strip_prefix('\u{feff}')
                .unwrap_or(patch.source);
            result.push(ResolvedPatchScript {
                id: patch.id.to_string(),
                file_name: patch.file_name.to_string(),
                label: patch.label.to_string(),
                group: patch.group,
                source: stripped.to_string(),
            });
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_all_28_non_empty_unique_scripts_and_22_menu_options() {
        assert_eq!(DEFAULT_PATCH_SCRIPTS.len(), 4);
        assert_eq!(SELECTABLE_MOD_OPTIONS.len(), 22);

        let mut ids = HashSet::new();
        let mut file_names = HashSet::new();
        let mut total_scripts = 0usize;

        for patch in DEFAULT_PATCH_SCRIPTS {
            assert!(
                !patch.source.trim().is_empty(),
                "empty default: {}",
                patch.file_name
            );
            assert!(ids.insert(patch.id), "duplicate id: {}", patch.id);
            assert!(
                file_names.insert(patch.file_name),
                "duplicate file_name: {}",
                patch.file_name
            );
            total_scripts += 1;
        }

        for (idx, opt) in SELECTABLE_MOD_OPTIONS.iter().enumerate() {
            assert_eq!(opt.number, u32::try_from(idx + 1).unwrap());
            assert!(!opt.scripts.is_empty());
            for patch in opt.scripts {
                assert!(
                    !patch.source.trim().is_empty(),
                    "empty mod: {}",
                    patch.file_name
                );
                assert!(ids.insert(patch.id), "duplicate id: {}", patch.id);
                assert!(
                    file_names.insert(patch.file_name),
                    "duplicate file_name: {}",
                    patch.file_name
                );
                total_scripts += 1;
            }
        }

        assert_eq!(total_scripts, 28);
    }

    #[test]
    fn console_bridge_substitutes_port_placeholder_cleanly() {
        let console_raw = DEFAULT_PATCH_SCRIPTS
            .iter()
            .find(|p| p.id == CONSOLE_PATCH_ID)
            .expect("console default patch must exist");
        assert!(
            console_raw.source.contains(CONSOLE_PATCH_PORT_TOKEN),
            "raw console bridge script must retain __CONSOLE_PATCH_PORT__"
        );

        let plan = build_injection_plan(&[], 17893);
        assert_eq!(plan.len(), 4);
        assert_eq!(plan[0].id, CONSOLE_PATCH_ID);
        assert_eq!(plan[0].file_name, "コンソール連携.js");
        assert!(!plan[0].source.contains(CONSOLE_PATCH_PORT_TOKEN));
        assert!(plan[0].source.contains("17893"));
    }

    #[test]
    fn expands_bone_dagger_bundle_and_preserves_order() {
        let plan = build_injection_plan(&[1, 7, 13], 7893);
        assert_eq!(plan.len(), 4 + 1 + 3 + 1);
        let names: Vec<&str> = plan.iter().map(|p| p.file_name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "コンソール連携.js",
                "スペルチャージ修正.js",
                "死亡切断通知.js",
                "接続保護_乱入防止.js",
                "役職ランダム.js",
                "骨の短剣_本体.js",
                "骨の短剣_ドロップ.js",
                "骨の短剣_初期装備.js",
                "銃_弓同時所持.js",
            ]
        );
    }

    #[test]
    fn enforces_all_four_bidirectional_conflict_rules() {
        let pairs: Vec<(u32, u32)> = get_mod_conflict_pairs()
            .into_iter()
            .map(|(a, b)| (a.number, b.number))
            .collect();
        assert_eq!(pairs, vec![(3, 21), (4, 22), (12, 14), (21, 22)]);

        for &(a_num, b_num) in &[(3, 21), (4, 22), (12, 14), (21, 22)] {
            let a = find_option_by_number(a_num).unwrap();
            let b = find_option_by_number(b_num).unwrap();
            assert!(are_mod_options_conflicting(a, b));
            assert!(are_mod_options_conflicting(b, a));
            assert_eq!(find_conflicting_selected_options(a, &[b_num]).len(), 1);
            assert_eq!(find_conflicting_selected_options(b, &[a_num]).len(), 1);
        }

        let plan = build_injection_plan(&[3, 21, 12, 14, 22, 4], 7893);
        let ids: Vec<&str> = plan.iter().skip(4).map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "single:resources_spawner_Departure_win64",
                "single:blizzard_armory_open_win64",
                "single:sightseeing_expanse_win64",
            ]
        );
    }
}
