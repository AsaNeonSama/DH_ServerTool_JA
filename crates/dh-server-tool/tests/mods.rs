use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use dh_server_tool::languages;
use dh_server_tool::mods::{
    are_mod_options_conflicting, build_injection_plan, find_conflicting_selected_options,
    find_content_option_by_number, find_option_by_number, get_mod_conflict_pairs,
    prepare_injection_plan, CONSOLE_PATCH_ID, CONSOLE_PATCH_PORT_TOKEN,
    CONTENT_MOD_ASSET_ROOT_TOKEN, DEFAULT_PATCH_SCRIPTS, SELECTABLE_CONTENT_MOD_OPTIONS,
    SELECTABLE_MOD_OPTIONS,
};

#[test]
fn embeds_all_30_non_empty_unique_scripts_and_22_menu_options() {
    assert_eq!(DEFAULT_PATCH_SCRIPTS.len(), 5);
    assert_eq!(SELECTABLE_MOD_OPTIONS.len(), 22);
    assert_eq!(SELECTABLE_CONTENT_MOD_OPTIONS.len(), 1);

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

    for opt in SELECTABLE_CONTENT_MOD_OPTIONS {
        assert!(!opt.scripts.is_empty());
        for patch in opt.scripts {
            assert!(
                !patch.source.trim().is_empty(),
                "empty content mod: {}",
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

    assert_eq!(total_scripts, 30);

    let cannibal_climb = &DEFAULT_PATCH_SCRIPTS[4];
    assert_eq!(cannibal_climb.id, "default:cannibal_climb_win64");
    assert_eq!(cannibal_climb.file_name, "食人鬼登攀補強.js");
    assert_eq!(
        languages::mod_label(cannibal_climb.label_code),
        "食人鬼登攀補強"
    );
    assert!(
        cannibal_climb.source.contains("0xE9F130")
            && cannibal_climb.source.contains("0x242E4E0")
            && cannibal_climb.source.contains("0xD4CA50")
            && cannibal_climb.source.contains("0x22E1260")
            && cannibal_climb.source.contains("0x2D7A130")
            && cannibal_climb.source.contains("0x241D1E0")
            && cannibal_climb.source.contains("0x241D5A0")
            && cannibal_climb.source.contains("0x2400340"),
        "default cannibal climb script must reference ADH_CannibalCharacter (0xE9F130), SetWalkableFloorAngle (0x242E4E0), CharacterStuck (0xD4CA50), K2_SetActorRotation (0x22E1260), StopMovement (0x2D7A130), IsWalkable (0x241D1E0), K2_FindFloor (0x241D5A0), and LineTraceSingleByChannel (0x2400340)"
    );
    assert!(
        !cannibal_climb.source.contains("0x241FA20"),
        "PackNetworkMovementMode (0x241FA20) swim disguise must stay removed from 食人鬼登攀補強.js"
    );

    let all_sabotage = find_option_by_number(5).expect("mod #5 must exist");
    assert_eq!(all_sabotage.id, "spell_custom");
    assert_eq!(
        languages::mod_label(all_sabotage.label_code),
        "全サボタージュ"
    );
    assert_eq!(all_sabotage.scripts[0].file_name, "全サボタージュ.js");
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
    assert_eq!(plan.len(), 5);
    assert_eq!(plan[0].id, CONSOLE_PATCH_ID);
    assert_eq!(plan[0].file_name, "コンソール連携.js");
    assert!(!plan[0].source.contains(CONSOLE_PATCH_PORT_TOKEN));
    assert!(plan[0].source.contains("17893"));
}

#[test]
fn expands_bone_dagger_bundle_and_preserves_order() {
    let plan = build_injection_plan(&[1, 7, 13], 7893);
    assert_eq!(plan.len(), 5 + 1 + 3 + 1);
    let names: Vec<&str> = plan.iter().map(|p| p.file_name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "コンソール連携.js",
            "スペルチャージ修正.js",
            "死亡切断通知.js",
            "接続保護_乱入防止.js",
            "食人鬼登攀補強.js",
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
    let ids: Vec<&str> = plan.iter().skip(5).map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "single:resources_spawner_Departure_win64",
            "single:blizzard_armory_open_win64",
            "single:sightseeing_expanse_win64",
        ]
    );
}

#[test]
fn embeds_zero_asset_totem_spells_content_mod_and_orders_before_server_only_mods() {
    assert_eq!(SELECTABLE_CONTENT_MOD_OPTIONS.len(), 1);
    let opt = find_content_option_by_number(1).expect("content mod #1 must exist");
    assert_eq!(opt.id, "content_totem_spells");
    assert_eq!(opt.scripts.len(), 1);
    let script = &opt.scripts[0];
    assert!(
        script.asset_bundle.is_none(),
        "Option C pure-JS totem spells server script must not embed server-side .uasset bundles"
    );
    assert!(
        !script.source.contains(CONTENT_MOD_ASSET_ROOT_TOKEN),
        "Option C pure-JS totem spells script must not reference __CONTENT_MOD_ASSET_ROOT__"
    );
    for class_name in [
        "TS_BombMice_C",
        "TS_BoomShip_C",
        "TS_Teleport_C",
        "TS_Intimidate_C",
        "TS_Transfer_C",
    ] {
        assert!(
            script.source.contains(class_name),
            "missing synthetic spell class: {class_name}"
        );
    }
    assert!(
        script.source.contains("intimidateRadiusUU: 5000.0"),
        "TS_Intimidate radius must be 50m (5000.0 UU)"
    );
    assert!(
        script.source.contains("0x2AA7C70") && script.source.contains("0xD5E0D0"),
        "TS_Intimidate must bind ClientSetCameraFade (0x2AA7C70) and ModifyStamina (0xD5E0D0)"
    );
    assert!(
        script.source.contains("bombMiceExplosionDamage: 7.0")
            && script.source.contains("0x25570C0")
            && script.source.contains("0x255FCF0"),
        "TS_BombMice must use damage 7.0 with zero-CDO-mutation deferred explosion spawn (0x25570C0 / 0x255FCF0)"
    );
    assert!(
        script.source.contains("0xE84190")
            && script.source.contains("0x2DE69C0")
            && script.source.contains("0x2D70720")
            && script.source.contains("0x2C91A40")
            && script.source.contains("0x242E4E0")
            && script.source.contains("0x22E1260")
            && script.source.contains("0x241D1E0")
            && script.source.contains("0x241D5A0")
            && script.source.contains("0x2400340"),
        "TS_BombMice must hook SpawnAI (0xE84190) and use pure-JS AAIController NavMesh pathfinding + vertical wall climbing + 3-layer ship/gangway guard (0x2DE69C0 / 0x2D70720 / 0x2C91A40 / 0x242E4E0 / 0x22E1260 / 0x241D1E0 / 0x241D5A0 / 0x2400340)"
    );
    assert!(
        !script.source.contains("0x241FA20"),
        "PackNetworkMovementMode (0x241FA20) swim disguise must stay absent from カスタムスペル.js"
    );

    let temp_cache: PathBuf =
        std::env::temp_dir().join(format!("dh_content_mod_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_cache);

    // Injection order: 5 default -> Content MODs -> Server-only JS MODs
    let plan = prepare_injection_plan(&[1], &[1, 13], 7893, Some(&temp_cache))
        .expect("prepare_injection_plan must succeed");
    let names: Vec<&str> = plan.iter().map(|p| p.file_name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "コンソール連携.js",
            "スペルチャージ修正.js",
            "死亡切断通知.js",
            "接続保護_乱入防止.js",
            "食人鬼登攀補強.js",
            "カスタムスペル.js",
            "役職ランダム.js",
            "銃_弓同時所持.js",
        ]
    );
    assert!(
        !temp_cache.exists(),
        "zero-asset Option C Content MOD should not create any server asset cache directory"
    );
}

#[test]
fn console_bridge_handles_per_player_setspellcharge_directly() {
    let console_raw = DEFAULT_PATCH_SCRIPTS
        .iter()
        .find(|p| p.id == CONSOLE_PATCH_ID)
        .expect("console default patch must exist");
    assert!(
        console_raw.source.contains("cmdName === 'setspellcharge'"),
        "console bridge must intercept setspellcharge per player"
    );
    assert!(
        console_raw.source.contains("0xE82CC0"),
        "console bridge must bind ADH_SpellManager::SetSpellChargeLevel (0xE82CC0)"
    );
    assert!(
        console_raw.source.contains("cmdName === 'endmatch'"),
        "console bridge must intercept endmatch with victory side selection"
    );
    assert!(
        console_raw.source.contains("0xDA9C30"),
        "console bridge must bind ADH_GameState::SetMatchResult (0xDA9C30)"
    );
    assert!(
        console_raw.source.contains("BP_HumanCharacter_Thrall_C"),
        "console bridge must support summoning Thrall Marine (BP_HumanCharacter_Thrall_C)"
    );
    assert!(
        console_raw.source.contains("0x25570C0") && console_raw.source.contains("0x255FCF0"),
        "console bridge must use deferred PlayerState spawn so summoned Marines stay out of PlayerArray"
    );
    assert!(
        console_raw.source.contains("0xE81F00")
            && console_raw.source.contains("snapshotSpellManagerEquippedSpells")
            && console_raw.source.contains("restoreSpellManagerEquippedSpells"),
        "console bridge must snapshot and restore EquippedSpells via SetEquippedSpells (0xE81F00) across Thrall toggles"
    );
}
