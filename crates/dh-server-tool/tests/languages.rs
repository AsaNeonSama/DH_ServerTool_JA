use dh_server_tool::colors;
use dh_server_tool::consts::web_console::PLAYER_NAME_LIST;
use dh_server_tool::languages::{
    actor_display_name, api_text, app_text, player_role_name, render_app, render_cli, render_server,
};
use dh_server_tool::structs::languages::{
    ApiCode, AppMsg, AppMsgCode, CliMsg, ConfigSummaryParams, ServerMsg,
};
use serde_json::Value;

#[test]
fn frontend_ja_language_pack_matches_rust_api_codes_and_roles() {
    let raw_json = include_str!("../../../assets/languages/ja.json");
    let parsed: Value = serde_json::from_str(raw_json).expect("ja.json must be valid JSON");
    assert_eq!(parsed["locale"], "ja");

    let api_obj = parsed["api"].as_object().expect("ja.json api object");
    let all_api_codes = [
        ApiCode::Success,
        ApiCode::NetworkParamError,
        ApiCode::ServerExecutionError,
        ApiCode::InvalidValueError,
        ApiCode::HighValueWarning,
        ApiCode::BroadcastPartialError,
    ];
    for code in all_api_codes {
        let serialized = serde_json::to_value(code).expect("ApiCode must serialize");
        let key = serialized.as_str().expect("ApiCode must serialize as str");
        let frontend_text = api_obj
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("Missing API code {key} in assets/languages/ja.json"));
        assert_eq!(frontend_text, api_text(code));
    }
    assert!(api_obj
        .get("API_NETWORK_TIMEOUT")
        .and_then(Value::as_str)
        .is_some());
    assert!(api_obj
        .get("API_UNKNOWN_ERROR")
        .and_then(Value::as_str)
        .is_some());

    let roles_obj = parsed["ui"]["roles"]
        .as_object()
        .expect("ja.json ui.roles object");
    for role in PLAYER_NAME_LIST {
        let frontend_role = roles_obj
            .get(role)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("Missing role {role} in assets/languages/ja.json"));
        assert_eq!(Some(frontend_role), player_role_name(role));
    }

    let items_obj = parsed["ui"]["items"]
        .as_object()
        .expect("ja.json ui.items object");
    for (bp_path, label_val) in items_obj {
        let frontend_label = label_val.as_str().expect("item label must be string");
        assert_eq!(
            actor_display_name(bp_path),
            Some(frontend_label),
            "Item mismatch between ja.json and Rust languages for {bp_path}"
        );
    }
}

#[test]
fn renders_japanese_port_switching_and_fixed_port_messages() {
    let summary_switch = render_app(&AppMsg::ConfigSummary(ConfigSummaryParams {
        server_port_1: 7895,
        server_port_2: 7897,
        switch_port_on_restart: true,
        web_console_port: 7891,
        console_patch_port: 7893,
        check_interval_text: "1s",
        restart_wait_seconds: 10,
    }));
    assert!(summary_switch.contains("7895/7897(切替オン)"));

    let summary_fixed = render_app(&AppMsg::ConfigSummary(ConfigSummaryParams {
        server_port_1: 7895,
        server_port_2: 7897,
        switch_port_on_restart: false,
        web_console_port: 7891,
        console_patch_port: 7893,
        check_interval_text: "1s",
        restart_wait_seconds: 10,
    }));
    assert!(summary_fixed.contains("7895(固定/切替オフ)"));

    let reconfig_switch = render_cli(&CliMsg::ReconfigurePreservedNotice {
        port_1: 7895,
        port_2: 7897,
        switch_port_on_restart: true,
    });
    assert!(reconfig_switch.contains("7895 <-> 7897"));

    let reconfig_fixed = render_cli(&CliMsg::ReconfigurePreservedNotice {
        port_1: 7895,
        port_2: 7895,
        switch_port_on_restart: false,
    });
    assert!(reconfig_fixed.contains("7895 固定"));

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

    let notice_fixed = render_server(&ServerMsg::RetryNotice {
        error_text: "timeout",
        switch_port_on_restart: false,
    });
    assert!(notice_fixed.contains("同ポートで再試行"));

    assert_eq!(app_text(AppMsgCode::UsageNoticeLabel), "使用上の注意:");
    assert_eq!(
        colors::APP_LICENSE_URL,
        "https://github.com/AsaNeonSama/DH_ServerTool_JA/blob/main/LICENSE"
    );
}
