use dh_server_tool::cli::flows::{
    parse_toggle_input, validate_server_ports, validate_single_server_port, validate_web_port,
    LaunchPlan, WebConsoleSetupConfig,
};
use dh_server_tool::cli::match_setup::{
    build_ue_map_argument, parse_map_choice, parse_number_in_range, parse_u32_in_range,
    MatchConfig, ServerSettings,
};
use dh_server_tool::cli::mod_selector::{
    apply_content_mod_selection_input, apply_mod_selection_input, build_content_mod_menu_lines,
    build_mod_menu_lines, ModSelectionCommand,
};
use dh_server_tool::colors;
use dh_server_tool::languages;
use dh_server_tool::structs::languages::{CliErrorCode, CliMsgCode};
use dh_server_tool::terminal::display::strip_ansi;

#[test]
fn validates_web_and_server_port_collisions() {
    assert_eq!(validate_web_port("", 7891, 7893), Ok(7891));
    assert_eq!(
        validate_web_port("7893", 7891, 7893),
        Err(CliErrorCode::WebPortConflictsWithPatch { patch_port: 7893 })
    );
    assert!(matches!(
        validate_web_port("80", 7891, 7893),
        Err(CliErrorCode::PortOutOfRange { .. })
    ));
    assert!(matches!(
        validate_web_port("70000", 7891, 7893),
        Err(CliErrorCode::PortOutOfRange { .. })
    ));

    let web_cfg = WebConsoleSetupConfig {
        web_console_port: 7891,
        console_patch_port: 7893,
        auto_broadcast: true,
    };
    assert_eq!(
        validate_server_ports("", "", [7895, 7897], web_cfg),
        Ok([7895, 7897])
    );
    assert_eq!(
        validate_server_ports("7895", "7895", [7895, 7897], web_cfg),
        Err(CliErrorCode::ServerPortsDuplicate)
    );
    assert_eq!(
        validate_server_ports("7891", "7897", [7895, 7897], web_cfg),
        Err(CliErrorCode::ServerPortConflictsWithWeb {
            web_port: 7891,
            patch_port: 7893,
        })
    );
    assert_eq!(
        validate_server_ports("7895", "7893", [7895, 7897], web_cfg),
        Err(CliErrorCode::ServerPortConflictsWithWeb {
            web_port: 7891,
            patch_port: 7893,
        })
    );
    assert!(matches!(
        validate_server_ports("1023", "7897", [7895, 7897], web_cfg),
        Err(CliErrorCode::PortOutOfRange { .. })
    ));

    assert_eq!(validate_single_server_port("", 7895, web_cfg), Ok(7895));
    assert_eq!(validate_single_server_port("9001", 7895, web_cfg), Ok(9001));
    assert_eq!(
        validate_single_server_port("7891", 7895, web_cfg),
        Err(CliErrorCode::ServerPortConflictsWithWeb {
            web_port: 7891,
            patch_port: 7893,
        })
    );
    assert_eq!(
        validate_single_server_port("7893", 7895, web_cfg),
        Err(CliErrorCode::ServerPortConflictsWithWeb {
            web_port: 7891,
            patch_port: 7893,
        })
    );
    assert!(matches!(
        validate_single_server_port("80", 7895, web_cfg),
        Err(CliErrorCode::PortOutOfRange { .. })
    ));
}

#[test]
fn parses_toggle_inputs_and_preserves_switch_port_mode_in_launch_plan() {
    assert_eq!(parse_toggle_input("", true), Some(true));
    assert_eq!(parse_toggle_input("", false), Some(false));
    assert_eq!(parse_toggle_input("1", false), Some(true));
    assert_eq!(parse_toggle_input("0", true), Some(false));
    assert_eq!(parse_toggle_input(" 0 ", true), Some(false));
    assert_eq!(parse_toggle_input("2", true), None);
    assert_eq!(parse_toggle_input("yes", false), None);

    let web_cfg = WebConsoleSetupConfig {
        web_console_port: 7891,
        console_patch_port: 7893,
        auto_broadcast: true,
    };
    let match_cfg = MatchConfig {
        map_id: "Departure_Persistent",
        map_label_code: CliMsgCode::MapLabel1,
        settings: ServerSettings::default(),
        selected_content_option_numbers: vec![1],
        selected_content_option_labels: vec!["カスタムスペル(新呪文5種)".to_string()],
        selected_option_numbers: vec![],
        selected_option_labels: vec![],
        ue_map_argument: build_ue_map_argument("Departure_Persistent", &ServerSettings::default()),
    };
    let fixed_plan = LaunchPlan::from_parts(web_cfg, [7895, 7895], false, match_cfg.clone());
    assert!(!fixed_plan.switch_port_on_restart);
    assert_eq!(fixed_plan.server_ports, [7895, 7895]);

    let updated = fixed_plan.with_match_config(match_cfg);
    assert!(!updated.switch_port_on_restart);
    assert_eq!(updated.server_ports, [7895, 7895]);
}

#[test]
fn parses_all_three_maps_and_rejects_invalid() {
    let m1 = parse_map_choice("1").unwrap();
    assert_eq!(m1.id, "Departure_Persistent");
    assert_eq!(languages::cli_text(m1.label_code), "山頂");

    let m2 = parse_map_choice("2").unwrap();
    assert_eq!(m2.id, "Expanse_Persistent");
    assert_eq!(languages::cli_text(m2.label_code), "広漠");

    let m3 = parse_map_choice("3").unwrap();
    assert_eq!(m3.id, "Approach_Persistent");
    assert_eq!(languages::cli_text(m3.label_code), "進入路");

    assert!(parse_map_choice("0").is_none());
    assert!(parse_map_choice("4").is_none());
    assert!(parse_map_choice("").is_none());
}

#[test]
fn builds_ue_map_argument_matching_ts_format() {
    let defaults = ServerSettings::default();
    assert_eq!(
        build_ue_map_argument("Departure_Persistent", &defaults),
        "Departure_Persistent?maxplayers=8?daysbeforeblizzard=3?dayminutes=9?predatordamage=1?coldintensity=1?hungerrate=1?coalburnrate=1?thralls=2"
    );

    let custom = ServerSettings {
        max_players: 6,
        days_before_blizzard: 4,
        day_minutes: 10,
        predator_damage: 0.75,
        cold_intensity: 1.5,
        hunger_rate: 1.25,
        coal_burn_rate: 0.5,
        thralls: 1,
    };
    assert_eq!(
        build_ue_map_argument("Expanse_Persistent", &custom),
        "Expanse_Persistent?maxplayers=6?daysbeforeblizzard=4?dayminutes=10?predatordamage=0.75?coldintensity=1.5?hungerrate=1.25?coalburnrate=0.5?thralls=1"
    );
}

#[test]
fn validates_integer_and_float_ranges() {
    assert_eq!(parse_u32_in_range("", 1, 8, 8), Some(8));
    assert_eq!(parse_u32_in_range("1", 1, 8, 8), Some(1));
    assert_eq!(parse_u32_in_range("8", 1, 8, 8), Some(8));
    assert_eq!(parse_u32_in_range("0", 1, 8, 8), None);
    assert_eq!(parse_u32_in_range("9", 1, 8, 8), None);
    assert_eq!(parse_u32_in_range("3.5", 1, 8, 8), None);

    assert_eq!(parse_number_in_range("", 1.0, 8.0, 8.0, true), Some(8.0));
    assert_eq!(parse_number_in_range("1", 1.0, 8.0, 8.0, true), Some(1.0));
    assert_eq!(parse_number_in_range("8", 1.0, 8.0, 8.0, true), Some(8.0));
    assert_eq!(parse_number_in_range("0", 1.0, 8.0, 8.0, true), None);
    assert_eq!(parse_number_in_range("9", 1.0, 8.0, 8.0, true), None);
    assert_eq!(parse_number_in_range("3.5", 1.0, 8.0, 8.0, true), None);
    assert_eq!(
        parse_number_in_range("0.25", 0.25, 3.0, 1.0, false),
        Some(0.25)
    );
    assert_eq!(
        parse_number_in_range("0.1", 0.1, 2.0, 1.0, false),
        Some(0.1)
    );
    assert_eq!(parse_number_in_range("0.09", 0.1, 2.0, 1.0, false), None);
    assert_eq!(parse_number_in_range("NaN", 0.1, 2.0, 1.0, false), None);
    assert_eq!(parse_number_in_range("inf", 0.1, 2.0, 1.0, false), None);
    assert_eq!(parse_number_in_range("abc", 0.1, 2.0, 1.0, false), None);
}

#[test]
fn batch_input_supports_ascii_fullwidth_and_japanese_commas() {
    let mut selected = Vec::new();
    let res = apply_mod_selection_input(&mut selected, "1, 2，7、13");
    assert_eq!(selected, vec![1, 2, 7, 13]);
    match res {
        ModSelectionCommand::Processed(step) => {
            assert_eq!(step.added_labels.len(), 4);
            assert_eq!(step.duplicate_labels, Vec::<&str>::new());
            assert_eq!(step.conflict_notices, Vec::<String>::new());
            assert_eq!(step.invalid_tokens, Vec::<String>::new());
        }
        other => panic!("unexpected command: {other:?}"),
    }

    let dup = apply_mod_selection_input(&mut selected, "7, 99, abc");
    assert_eq!(selected, vec![1, 2, 7, 13]);
    match dup {
        ModSelectionCommand::Processed(step) => {
            assert_eq!(step.added_labels, Vec::<&str>::new());
            assert_eq!(step.duplicate_labels, vec!["骨の短剣モード"]);
            assert_eq!(step.invalid_tokens, vec!["99", "abc"]);
        }
        other => panic!("unexpected command: {other:?}"),
    }

    assert_eq!(
        apply_mod_selection_input(&mut selected, "C"),
        ModSelectionCommand::Cleared
    );
    assert_eq!(selected, Vec::<u32>::new());
    assert_eq!(
        apply_mod_selection_input(&mut selected, "y"),
        ModSelectionCommand::Confirm
    );
    assert_eq!(
        apply_mod_selection_input(&mut selected, "  , 、 "),
        ModSelectionCommand::EmptyInvalid
    );
}

#[test]
fn enforces_all_four_bidirectional_conflict_rules_in_both_directions() {
    let pairs = [(3u32, 21u32), (4, 22), (12, 14), (21, 22)];
    for (a, b) in pairs {
        let mut sel_fwd = Vec::new();
        let res_fwd = apply_mod_selection_input(&mut sel_fwd, &format!("{a},{b}"));
        assert_eq!(sel_fwd, vec![a], "forward conflict failed for {a}<->{b}");
        match res_fwd {
            ModSelectionCommand::Processed(step) => {
                assert_eq!(step.added_labels.len(), 1);
                assert_eq!(step.conflict_notices.len(), 1);
            }
            other => panic!("unexpected forward result: {other:?}"),
        }

        let mut sel_rev = Vec::new();
        let _ = apply_mod_selection_input(&mut sel_rev, &b.to_string());
        let res_rev = apply_mod_selection_input(&mut sel_rev, &a.to_string());
        assert_eq!(sel_rev, vec![b], "reverse conflict failed for {b}<->{a}");
        match res_rev {
            ModSelectionCommand::Processed(step) => {
                assert_eq!(step.added_labels, Vec::<&str>::new());
                assert_eq!(step.conflict_notices.len(), 1);
            }
            other => panic!("unexpected reverse result: {other:?}"),
        }
    }
}

#[test]
fn renders_multi_column_menu_with_bone_dagger_suffix_and_conflict_legend() {
    let lines = build_mod_menu_lines();
    assert!(
        lines.first().is_some_and(|l| l.starts_with(colors::CYAN)),
        "server-only MOD menu lines should keep CYAN color"
    );
    let plain = lines
        .iter()
        .map(|l| strip_ansi(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(plain.contains("7:骨の短剣モード [セット]"));
    assert!(plain.contains("3:マップ補強(山頂) <-> 21:観光モード(山頂)"));
    assert!(plain.contains("4:マップ補強(広漠) <-> 22:観光モード(広漠)"));
    assert!(plain.contains("12:暴風雪マスケット <-> 14:開幕マスケット"));
    assert!(plain.contains("21:観光モード(山頂) <-> 22:観光モード(広漠)"));
}

#[test]
fn renders_and_processes_content_mod_selection_separately_from_server_only_mods() {
    let lines = build_content_mod_menu_lines();
    assert!(
        lines
            .first()
            .is_some_and(|l| l.starts_with(colors::LIGHT_MAGENTA)),
        "PAK-required Content MOD menu lines must render in LIGHT_MAGENTA"
    );
    assert!(
        lines.first().is_some_and(|l| !l.starts_with(colors::CYAN)),
        "PAK-required Content MOD menu lines must not use CYAN"
    );
    let plain = lines
        .iter()
        .map(|l| strip_ansi(l))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(plain.contains("1:カスタムスペル(新呪文5種) [PAK必須]"));
    assert!(!plain.contains("役職ランダム"));

    let mut selected = Vec::new();
    let added = apply_content_mod_selection_input(&mut selected, "1");
    assert_eq!(selected, vec![1]);
    match added {
        ModSelectionCommand::Processed(step) => {
            assert_eq!(step.added_labels, vec!["カスタムスペル(新呪文5種)"]);
            assert_eq!(step.invalid_tokens, Vec::<String>::new());
        }
        other => panic!("unexpected command: {other:?}"),
    }

    let invalid = apply_content_mod_selection_input(&mut selected, "2");
    assert_eq!(selected, vec![1]);
    match invalid {
        ModSelectionCommand::Processed(step) => {
            assert_eq!(step.invalid_tokens, vec!["2"]);
        }
        other => panic!("unexpected command: {other:?}"),
    }

    assert_eq!(
        apply_content_mod_selection_input(&mut selected, "c"),
        ModSelectionCommand::Cleared
    );
    assert_eq!(selected, Vec::<u32>::new());
    assert_eq!(
        apply_content_mod_selection_input(&mut selected, "y"),
        ModSelectionCommand::Confirm
    );
}
