use dh_server_tool::consts::web_console::PLAYER_NAME_LIST;
use dh_server_tool::languages::{actor_display_name, api_text, player_role_name};
use dh_server_tool::structs::languages::ApiCode;
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
