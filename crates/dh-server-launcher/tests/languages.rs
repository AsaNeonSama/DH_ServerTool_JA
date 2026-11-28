use dh_server_launcher::consts::console::PLAYER_NAME_LIST;
use dh_server_launcher::consts::gui::{
    APP_LICENSE_URL, CANNIBAL_RANDOM_KEY, CONSOLE_ITEM_PATHS, CONSOLE_SUMMON_KEYS,
};
use dh_server_launcher::languages::{
    actor_display_name, api_text, gui_text, mod_description, player_role_name, render_app,
    render_gui, render_server, server_text,
};
use dh_server_launcher::structs::languages::{
    ApiCode, AppMsg, GuiMsg, GuiMsgCode, ModLabelCode, ServerMsg, ServerMsgCode,
};

#[test]
fn covers_all_api_codes_roles_summons_and_44_items_in_japanese() {
    let all_api_codes = [
        ApiCode::Success,
        ApiCode::NetworkParamError,
        ApiCode::ServerExecutionError,
        ApiCode::InvalidValueError,
        ApiCode::HighValueWarning,
        ApiCode::BroadcastPartialError,
    ];
    for code in all_api_codes {
        assert_ne!(api_text(code), "");
    }

    assert_eq!(PLAYER_NAME_LIST.len(), 8);
    for role in PLAYER_NAME_LIST {
        assert!(
            player_role_name(role).is_some_and(|label| !label.is_empty()),
            "Missing Japanese role label for {role}"
        );
    }

    assert_eq!(CONSOLE_SUMMON_KEYS.len(), 11);
    for key in CONSOLE_SUMMON_KEYS {
        if key == CANNIBAL_RANDOM_KEY {
            continue;
        }
        assert!(
            actor_display_name(key).is_some_and(|label| !label.is_empty()),
            "Missing Japanese summon label for {key}"
        );
    }

    assert_eq!(CONSOLE_ITEM_PATHS.len(), 44);
    for bp_path in CONSOLE_ITEM_PATHS {
        assert!(
            actor_display_name(bp_path).is_some_and(|label| !label.is_empty()),
            "Missing Japanese item label for {bp_path}"
        );
    }
}

#[test]
fn renders_japanese_server_app_gui_and_client_required_mod_messages() {
    let created = render_app(&AppMsg::ConfigCreated {
        relative_path: "./launcher_config.json",
    });
    assert!(created.contains("./launcher_config.json"));

    let load_failed = render_app(&AppMsg::ConfigLoadFailed {
        error_text: "parse error",
    });
    assert!(load_failed.contains("parse error"));

    assert_eq!(server_text(ServerMsgCode::MapLabel1), "山頂");
    assert_eq!(server_text(ServerMsgCode::MapLabel2), "広漠");
    assert_eq!(server_text(ServerMsgCode::MapLabel3), "進入路");

    let monitor_switch = render_server(&ServerMsg::MonitoringServer {
        interval_text: "1s",
        switch_port_on_restart: true,
    });
    assert!(monitor_switch.contains("自動ポート切替"));

    let monitor_fixed = render_server(&ServerMsg::MonitoringServer {
        interval_text: "1s",
        switch_port_on_restart: false,
    });
    assert!(monitor_fixed.contains("同ポートで自動再起動"));

    let retry_fixed = render_server(&ServerMsg::StartupRetryAfter {
        delay_text: "5s",
        switch_port_on_restart: false,
    });
    assert!(retry_fixed.contains("同じポートで再試行"));

    assert_ne!(gui_text(GuiMsgCode::ServerAlreadyRunning), "");
    assert_ne!(gui_text(GuiMsgCode::ServerStoppedCleanly), "");
    assert!(render_gui(&GuiMsg::ServerExeNotFound {
        path: "./missing.exe"
    })
    .contains("./missing.exe"));

    assert!(
        mod_description(ModLabelCode::ContentTotemSpells).is_some_and(|d| d.contains(".pak/.sig"))
    );
    assert!(mod_description(ModLabelCode::ContentUeVoice)
        .is_some_and(|d| d.contains("dh_voice_client.dll")));
    assert_eq!(
        APP_LICENSE_URL,
        "https://github.com/AsaNeonSama/DH_Tool_JA/blob/main/LICENSE"
    );
}
