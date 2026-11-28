use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dh_server_launcher::config::{
    load_or_create_config_at, parse_config_value, save_config_at, validate_app_config, AppConfig,
};
use dh_server_launcher::console::dispatch::render_console_log_record;
use dh_server_launcher::consts::console::status_code;
use dh_server_launcher::gui::console::{
    build_admin_console_catalog, dispatch_admin_console_action,
};
use dh_server_launcher::gui::state::{
    build_map_option_dtos, build_mod_catalog_dtos, build_patch_progress_list,
    build_validated_match_config, normalize_picked_exe_path,
};
use dh_server_launcher::mods::{
    build_injection_plan_with_content_mods, SELECTABLE_CONTENT_MOD_OPTIONS,
};
use dh_server_launcher::structs::console::{ConsoleBridge, ConsoleLogRecord, ConsoleLogSink};
use dh_server_launcher::structs::gui::{
    AdminConsoleActionRequestDto, GuiAppState, LaunchServerRequestDto, MatchSettingsDto,
    PatchProgressCategory, PatchProgressStatus,
};
use dh_server_launcher::structs::languages::{ApiCode, ConsoleLogEvent};
use dh_server_launcher::structs::server::ServerSettings;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use uuid::Uuid;

#[test]
fn partitions_mods_into_client_required_practical_and_fun_sections() {
    let catalog = build_mod_catalog_dtos();
    assert_eq!(catalog.default_mods.len(), 5);
    assert_eq!(
        catalog.client_required_mods.len(),
        SELECTABLE_CONTENT_MOD_OPTIONS.len()
    );
    assert_eq!(catalog.client_required_mods.len(), 2);

    assert_eq!(catalog.client_required_mods[0].id, "content_totem_spells");
    assert_eq!(
        catalog.client_required_mods[0].requirement_badge.as_deref(),
        Some("[PAK必須]")
    );
    assert_eq!(
        catalog.client_required_mods[0].description.as_deref(),
        Some("新呪文5種を追加します。クライアント側へ対応 .pak/.sig の導入が必須です。")
    );

    assert_eq!(catalog.client_required_mods[1].id, "content_ue_voice");
    assert_eq!(catalog.client_required_mods[1].label, "自有音声サーバー");
    assert_eq!(
        catalog.client_required_mods[1].requirement_badge.as_deref(),
        Some("[DLL必須]")
    );
    assert_eq!(
        catalog.client_required_mods[1]
            .requirement_detail
            .as_deref(),
        Some("dh_voice_client.dll")
    );
    assert_eq!(
        catalog.client_required_mods[1].scripts,
        vec!["dh_voice_server.dll"]
    );
    assert_eq!(
        catalog.client_required_mods[1].description.as_deref(),
        Some("クライアント側で「自有音声クライアント」を有効化してください。")
    );

    assert_eq!(catalog.practical_mods.len(), 13);
    assert_eq!(catalog.practical_mods[0].number, 1);
    assert_eq!(catalog.practical_mods[12].number, 13);

    assert_eq!(catalog.fun_mods.len(), 9);
    assert_eq!(catalog.fun_mods[0].number, 14);
    assert_eq!(catalog.fun_mods[8].number, 22);

    assert_eq!(catalog.conflict_pairs.len(), 4);

    let maps = build_map_option_dtos();
    assert_eq!(maps.len(), 3);
    assert_eq!(maps[0].id, "Departure_Persistent");
    assert_eq!(maps[1].id, "Expanse_Persistent");
    assert_eq!(maps[2].id, "Approach_Persistent");
}

#[test]
fn exposes_full_in_app_admin_console_catalog_matching_dh_console() {
    let catalog = build_admin_console_catalog();
    assert_eq!(catalog.roles.len(), 8);
    assert_eq!(catalog.command_types.len(), 8);
    assert_eq!(catalog.summons.len(), 11);
    assert_eq!(catalog.spell_charges.len(), 3);
    assert_eq!(catalog.blizzard_intensities.len(), 5);
    assert_eq!(catalog.slomos.len(), 5);
    assert_eq!(catalog.statuses.len(), 13);
    assert_eq!(catalog.items.len(), 44);
    assert_eq!(catalog.item_counts, vec![1, 3, 5, 10, 20]);
    assert_eq!(catalog.storages.len(), 3);

    // MinStamina must map cleanly to MinStamina (not the legacy MaximumStamina copy-paste)
    let min_stamina = catalog
        .statuses
        .iter()
        .find(|s| s.value == "MinStamina")
        .expect("MinStamina status must exist");
    assert!(!min_stamina.is_toggle);
    assert_eq!(min_stamina.label, "最小スタミナ");
}

#[test]
fn validates_match_config_and_builds_ue_argument() {
    let valid_req = LaunchServerRequestDto {
        map_id: "Expanse_Persistent".to_string(),
        settings: MatchSettingsDto::from(ServerSettings::default()),
        selected_content_numbers: vec![1, 2],
        selected_option_numbers: vec![1, 7],
        auto_restart_watchdog: true,
    };
    let match_cfg = build_validated_match_config(&valid_req).expect("valid match request");
    assert_eq!(match_cfg.map_id, "Expanse_Persistent");
    assert_eq!(match_cfg.selected_content_option_numbers, vec![1, 2]);
    assert_eq!(match_cfg.selected_option_numbers, vec![1, 7]);
    assert!(match_cfg
        .ue_map_argument
        .starts_with("Expanse_Persistent?maxplayers=8"));

    let mut bad_map = valid_req.clone();
    bad_map.map_id = "Unknown_Map".to_string();
    assert!(build_validated_match_config(&bad_map).is_err());

    let mut bad_players = valid_req.clone();
    bad_players.settings.max_players = 99;
    assert!(build_validated_match_config(&bad_players).is_err());

    let mut bad_coal = valid_req;
    bad_coal.settings.coal_burn_rate = 0.01;
    assert!(build_validated_match_config(&bad_coal).is_err());
}

#[test]
fn normalizes_picked_executable_paths_relative_to_cwd() {
    let base = Path::new(r"C:\Game\ServerRoot");
    let inside = Path::new(
        r"C:\Game\ServerRoot\DreadHunger\Binaries\Win64\DreadHungerServer-Win64-Shipping.exe",
    );
    assert_eq!(
        normalize_picked_exe_path(inside, Some(base)),
        "./DreadHunger/Binaries/Win64/DreadHungerServer-Win64-Shipping.exe"
    );

    let outside = Path::new(r"D:\OtherServer\DreadHungerServer-Win64-Shipping.exe");
    assert_eq!(
        normalize_picked_exe_path(outside, Some(base)),
        "D:/OtherServer/DreadHungerServer-Win64-Shipping.exe"
    );
}

#[test]
fn records_ansi_free_console_logs_and_preserves_ui_draft_in_state_snapshots() {
    let cmd_rec = render_console_log_record(&ConsoleLogEvent::CommandResult {
        role_key: "Captain",
        command: "setisthrall 1",
        added_bone_dagger: true,
        status: status_code::SUCCESS,
        err_detail: None,
    });
    assert_eq!(cmd_rec.category, "コマンド");
    assert_eq!(cmd_rec.tone, "info");
    assert!(!cmd_rec.detail.contains('\x1b'));
    assert!(cmd_rec.detail.contains("船長"));

    let kick_err = render_console_log_record(&ConsoleLogEvent::KickResult {
        role_key: "Cook",
        status: status_code::SERVER_ERROR,
        err_detail: Some("connection refused"),
    });
    assert_eq!(kick_err.category, "キック");
    assert_eq!(kick_err.tone, "error");
    assert!(!kick_err.detail.contains('\x1b'));
    assert!(kick_err.detail.contains("料理人"));

    // Verify backend GuiAppState retains buffered logs & UI draft across snapshot re-hydration
    let app_state = GuiAppState::initialize();
    let e1 = app_state.push_console_log(cmd_rec);
    let e2 = app_state.push_console_log(kick_err);
    assert_eq!(e1.id, 1);
    assert_eq!(e2.id, 2);

    {
        let mut guard = app_state.lock_shared();
        guard.ui_draft.active_tab = "mods".to_string();
        guard.ui_draft.match_draft.selected_content_numbers = vec![1, 2];
        guard.ui_draft.console_draft.chat_message = "テストメッセージ".to_string();
    }

    let snap = app_state.build_snapshot();
    assert_eq!(snap.console_logs.len(), 2);
    assert_eq!(snap.ui_draft.active_tab, "mods");
    assert_eq!(
        snap.ui_draft.match_draft.selected_content_numbers,
        vec![1, 2]
    );
    assert_eq!(snap.ui_draft.console_draft.chat_message, "テストメッセージ");
}

#[test]
fn saves_and_validates_config_preserving_backwards_compatibility() {
    let path: PathBuf =
        std::env::temp_dir().join(format!("dh_gui_save_config_{}.json", Uuid::new_v4()));

    // Legacy config files with webConsolePort and unknown webConsolePsk fields must still parse cleanly
    let legacy_json = serde_json::json!({
        "serverPorts": [8095, 8097],
        "switchPortOnRestart": true,
        "consolePatchPort": 8093,
        "webConsolePort": 7891,
        "webConsolePsk": "legacy-ignored-token",
        "autoBroadcastDefault": false
    });
    let parsed = parse_config_value(&legacy_json);
    assert_eq!(parsed.server_ports, [8095, 8097]);
    assert_eq!(parsed.console_patch_port, 8093);
    assert!(!parsed.auto_broadcast_default);

    let cfg = AppConfig {
        console_patch_port: 8093,
        server_ports: [8095, 8097],
        switch_port_on_restart: true,
        ..AppConfig::default()
    };

    let saved = save_config_at(&path, &cfg).expect("save valid config");
    assert_eq!(saved, cfg);

    let reloaded = load_or_create_config_at(&path);
    assert_eq!(reloaded.config, cfg);

    // Server port colliding with internal loopback consolePatchPort must fail validation
    let mut bad_cfg = cfg.clone();
    bad_cfg.server_ports[0] = bad_cfg.console_patch_port;
    assert!(validate_app_config(&bad_cfg).is_err());

    let mut dup_srv = cfg;
    dup_srv.server_ports = [8095, 8095];
    dup_srv.switch_port_on_restart = true;
    assert!(validate_app_config(&dup_srv).is_err());

    let _ = fs::remove_file(&path);
}

#[test]
fn classifies_patch_progress_categories_accurately() {
    let patches = build_injection_plan_with_content_mods(&[1, 2], &[1], 7893, None);
    let progress = build_patch_progress_list(&patches);
    assert_eq!(progress.len(), 7);
    assert_eq!(progress[0].category, PatchProgressCategory::Default);
    assert_eq!(progress[4].category, PatchProgressCategory::Default);
    assert_eq!(progress[5].category, PatchProgressCategory::ClientRequired);
    assert_eq!(progress[6].category, PatchProgressCategory::ServerOnly);
    assert!(progress
        .iter()
        .all(|p| p.status == PatchProgressStatus::Pending));
}

#[tokio::test]
async fn dispatches_in_app_admin_console_command_directly_over_loopback_bridge() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind mock loopback console patch port");
    let port = listener.local_addr().expect("local addr").port();

    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept packet");
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.expect("read UTF-16LE");
        assert!(raw.len() >= 2 && raw.len() % 2 == 0);
        let units: Vec<u16> = raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        assert_eq!(units.last().copied(), Some(0));
        String::from_utf16(&units[..units.len() - 1]).expect("valid UTF-16")
    });

    let logs: Arc<Mutex<Vec<ConsoleLogRecord>>> = Arc::new(Mutex::new(Vec::new()));
    let logs_clone = Arc::clone(&logs);
    let sink: ConsoleLogSink = Arc::new(move |record: ConsoleLogRecord| {
        logs_clone.lock().expect("lock logs").push(record);
    });

    let bridge = ConsoleBridge::new(port);
    let result = dispatch_admin_console_action(
        &bridge,
        &sink,
        &AdminConsoleActionRequestDto::ConsoleCommand {
            player_name: "Captain".to_string(),
            command: "summon CANNIBAL_RANDOM".to_string(),
            auto_broadcast: false,
        },
    )
    .await;

    assert!(result.ok);
    assert_eq!(result.status, 200);
    assert_eq!(result.code, ApiCode::Success);

    let received_json = server_task.await.expect("mock server result");
    let parsed: serde_json::Value =
        serde_json::from_str(&received_json).expect("valid JSON packet");
    assert_eq!(parsed["action"], "execute_console_command");
    assert_eq!(parsed["targetRole"], "Captain");
    let cmd = parsed["command"].as_str().expect("command string");
    assert!(
        cmd == "summon BP_Cannibal_C"
            || cmd == "summon BP_Cannibal_Crawler_C"
            || cmd == "summon BP_Cannibal_Shambler_C",
        "CANNIBAL_RANDOM must resolve to a valid cannibal blueprint: {cmd}"
    );
    let logged = logs.lock().expect("lock logs");
    assert_eq!(logged.len(), 1);
    assert_eq!(logged[0].category, "コマンド");
    assert!(!logged[0].detail.contains('\x1b'));
}

#[test]
fn embeds_full_repository_license_text_at_compile_time() {
    let license = dh_server_launcher::consts::gui::EMBEDDED_LICENSE_TEXT;
    assert_ne!(license.trim(), "");
    assert!(license.contains("DH_Tool_JA"));
    assert!(license.contains("AsaNeonSama"));
}
