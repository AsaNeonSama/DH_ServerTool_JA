use std::collections::HashSet;

use dh_server_tool::mods::{
    are_mod_options_conflicting, build_injection_plan, find_conflicting_selected_options,
    find_option_by_number, get_mod_conflict_pairs, CONSOLE_PATCH_ID, CONSOLE_PATCH_PORT_TOKEN,
    DEFAULT_PATCH_SCRIPTS, SELECTABLE_MOD_OPTIONS,
};

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
