use std::sync::{Arc, Mutex as StdMutex};

use axum::body::Body;
use axum::http::header::{
    ACCEPT_ENCODING, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE,
};
use axum::http::{Method, Request as HttpRequest, StatusCode};
use axum::response::Response;
use axum::Router;
use dh_server_tool::consts::web_console::{auth_bootstrap_tokens, paths};
use dh_server_tool::crypto::{
    AesGcmSessionCipher, AUTH_PATH, AUTH_VERIFY_PATH, COOKIE_AAD, MAX_PENDING_CHALLENGES,
};
use dh_server_tool::structs::languages::{ApiCode, WebLogEvent};
use dh_server_tool::web_console::assets::build_auth_bootstrap_html;
use dh_server_tool::web_console::auth::{
    parse_cookies, ApiResultPayload, COOKIE_NAME, MAX_ENCRYPTED_BODY_BYTES,
};
use dh_server_tool::web_console::bridge::{
    build_console_broadcast_message, encode_utf16le_nul, ConsoleBridge, BONE_DAGGER_ITEM,
};
use dh_server_tool::web_console::logger::{
    format_console_command_preview, noop_log_sink, normalize_client_ip, render_web_log_event,
    WebLogSink,
};
use dh_server_tool::web_console::routes::status_code;
use dh_server_tool::web_console::{
    build_web_console_router, start_web_console, WebConsoleOptions, WebConsoleState,
};
use http_body_util::BodyExt;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tower::ServiceExt;
use uuid::Uuid;

#[test]
fn auth_bootstrap_html_loads_from_asset_template_and_substitutes_tokens() {
    let html = build_auth_bootstrap_html("sample_challenge_123");
    assert!(html.contains("globalScope.__dhCipher"));
    assert!(!html.contains("/*__DH_CIPHER_INLINE__*/"));
    assert!(html.contains(r#"var CHALLENGE = "sample_challenge_123";"#));
    assert!(html.contains(&format!(r#"var VERIFY_PATH = "{AUTH_VERIFY_PATH}";"#)));
    assert!(html.contains(&format!(r#"var UN_AUTH_PATH = "{}";"#, paths::UN_AUTH)));
    assert!(html.contains(&format!(r#"var HOME_PAGE_PATH = "{}";"#, paths::HOME_PAGE)));
    assert!(!html.contains(auth_bootstrap_tokens::CHALLENGE));
    assert!(!html.contains(auth_bootstrap_tokens::VERIFY_PATH));
    assert!(!html.contains(auth_bootstrap_tokens::UN_AUTH_PATH));
    assert!(!html.contains(auth_bootstrap_tokens::HOME_PAGE_PATH));
}

#[test]
fn normalizes_client_ips() {
    assert_eq!(normalize_client_ip(None), "unknown");
    assert_eq!(normalize_client_ip(Some("")), "unknown");
    assert_eq!(normalize_client_ip(Some("::1")), "127.0.0.1");
    assert_eq!(
        normalize_client_ip(Some("::ffff:192.168.1.5")),
        "192.168.1.5"
    );
    assert_eq!(normalize_client_ip(Some(" 203.0.113.9\r\n")), "203.0.113.9");
}

#[test]
fn formats_console_command_preview() {
    assert_eq!(
        format_console_command_preview("summon BP_Predator_Bear_C"),
        "summon 熊"
    );
    assert_eq!(
        format_console_command_preview("SUMMON BP_Predator_Bear_C"),
        "summon 熊"
    );
    assert_eq!(
        format_console_command_preview("summon BP_HumanCharacter_C"),
        "summon 海兵 (船員)"
    );
    assert_eq!(
        format_console_command_preview("summon BP_HumanCharacter_Thrall_C"),
        "summon 海兵 (傀儡)"
    );
    assert_eq!(
        format_console_command_preview("endmatch 1"),
        "endmatch 探検家勝利"
    );
    assert_eq!(
        format_console_command_preview("endmatch 0"),
        "endmatch 傀儡勝利"
    );
    assert_eq!(
        format_console_command_preview("EndMatch false"),
        "endmatch 傀儡勝利"
    );
    assert_eq!(
        format_console_command_preview("SetBlizzardIntensity 0.35"),
        "setblizzardintensity 0.35"
    );
    assert_eq!(
        format_console_command_preview("setisthrall 1"),
        "setisthrall 1"
    );
}

#[test]
fn renders_web_log_events_from_codes() {
    let cmd_line = render_web_log_event(&WebLogEvent::CommandResult {
        client_ip: "127.0.0.1",
        role_key: "Captain",
        command: "setisthrall 1",
        added_bone_dagger: true,
        status: status_code::SUCCESS,
        err_detail: None,
    });
    assert!(cmd_line.contains("[コマンド]"));
    assert!(cmd_line.contains("船長 -> setisthrall 1 (+骨の短剣)"));
    assert!(cmd_line.contains("[送信済]"));

    let copy_line = render_web_log_event(&WebLogEvent::UrlCopied {
        is_public: true,
        success: true,
    });
    assert!(copy_line.contains("[コピー]"));
    assert!(copy_line.contains("外部URLをクリップボードにコピーしました"));
}

#[test]
fn encodes_utf16le_with_nul_terminator() {
    let encoded = encode_utf16le_nul("A船");
    // 'A' = 0x0041, '船' = 0x8239, '\0' = 0x0000
    assert_eq!(encoded, vec![0x41, 0x00, 0x39, 0x82, 0x00, 0x00]);
}

#[test]
fn builds_console_broadcast_messages() {
    assert_eq!(
        build_console_broadcast_message("Captain", "summon", "BP_Predator_Wolf_C").as_deref(),
        Some("船長 が狼を召喚しました")
    );
    assert_eq!(
        build_console_broadcast_message("Captain", "SUMMON", "BP_Predator_Wolf_C").as_deref(),
        Some("船長 が狼を召喚しました")
    );
    assert_eq!(
        build_console_broadcast_message("Captain", "summon", "BP_HumanCharacter_C").as_deref(),
        Some("船長 が海兵 (船員)を召喚しました")
    );
    assert_eq!(
        build_console_broadcast_message("Captain", "summon", "BP_HumanCharacter_Thrall_C")
            .as_deref(),
        Some("船長 が海兵 (傀儡)を召喚しました")
    );
    assert_eq!(
        build_console_broadcast_message("Captain", "endmatch", "0"),
        None
    );
    assert_eq!(
        build_console_broadcast_message("Cook", "SetIsThrall", "1").as_deref(),
        Some("料理人 が闇の傀儡に悪墜ちしました")
    );
    assert_eq!(
        build_console_broadcast_message("Cook", "setisthrall", "0").as_deref(),
        Some("料理人 が船員に戻りました")
    );
    assert_eq!(
        build_console_broadcast_message("Navigator", "setspellcharge", "1.00").as_deref(),
        Some("航海士 がスペールチャージしました")
    );
    assert_eq!(
        build_console_broadcast_message("Doctor", "setblizzardintensity", "0.00").as_deref(),
        Some("医者 が暴風雪を停止しました")
    );
    assert_eq!(
        build_console_broadcast_message("Doctor", "SetBlizzardIntensity", "1").as_deref(),
        Some("医者 が暴風雪を呼びました")
    );
    assert_eq!(
        build_console_broadcast_message("Marine", "slomo", "1").as_deref(),
        Some("海兵 が世界を元に戻しました")
    );
    assert_eq!(
        build_console_broadcast_message("Marine", "slomo", "3").as_deref(),
        Some("海兵 が世界を加速しました")
    );
    assert_eq!(
        build_console_broadcast_message("Hunter", "god", "0").as_deref(),
        Some("猟師 が無敵状態を解除しました")
    );
    assert_eq!(
        build_console_broadcast_message("Hunter", "god", "1").as_deref(),
        Some("猟師 が無敵状態になりました")
    );
    assert_eq!(
        build_console_broadcast_message("Hunter", "unknowncmd", "1"),
        None
    );
}

#[tokio::test]
async fn sends_utf16le_json_over_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        socket.read_to_end(&mut buf).await.unwrap();
        buf
    });

    let bridge = ConsoleBridge::new(port);
    bridge
        .send_player_message("Captain", "テスト通知")
        .await
        .unwrap();

    let raw = server_task.await.unwrap();
    assert!(raw.len() >= 2);
    assert_eq!(&raw[raw.len() - 2..], &[0x00, 0x00]);
    let units: Vec<u16> = raw[..raw.len() - 2]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .collect();
    let text = String::from_utf16(&units).unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["action"], "send_thrall_message");
    assert_eq!(val["targetRole"], "Captain");
    assert_eq!(val["message"], "テスト通知");
}

async fn spawn_mock_bridge() -> (u16, Arc<StdMutex<Vec<Value>>>) {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) => panic!("bind mock bridge: {e}"),
    };
    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(e) => panic!("mock bridge local_addr: {e}"),
    };
    let received = Arc::new(StdMutex::new(Vec::new()));
    let rec_clone = Arc::clone(&received);

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let rec = Arc::clone(&rec_clone);
            tokio::spawn(async move {
                let mut buf = Vec::new();
                if socket.read_to_end(&mut buf).await.is_ok() && buf.len() >= 2 {
                    let units: Vec<u16> = buf[..buf.len() - 2]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|&c| u16::from_le_bytes(c))
                        .collect();
                    if let Ok(text) = String::from_utf16(&units) {
                        if let Ok(val) = serde_json::from_str::<Value>(&text) {
                            if let Ok(mut guard) = rec.lock() {
                                guard.push(val);
                            }
                        }
                    }
                }
            });
        }
    });

    (port, received)
}

fn build_test_app(
    bridge_port: u16,
    auto_broadcast: bool,
) -> (Router, Arc<WebConsoleState>, AesGcmSessionCipher) {
    let cipher = AesGcmSessionCipher::generate();
    let auth_id = Uuid::new_v4().to_string().to_uppercase();
    let state = Arc::new(WebConsoleState::new(
        auth_id,
        cipher.clone(),
        bridge_port,
        auto_broadcast,
        noop_log_sink(),
    ));
    let router = build_web_console_router(Arc::clone(&state));
    (router, state, cipher)
}

async fn response_text(res: Response) -> String {
    let collected = match res.into_body().collect().await {
        Ok(c) => c,
        Err(e) => panic!("collect response body: {e}"),
    };
    let bytes = collected.to_bytes();
    match String::from_utf8(bytes.to_vec()) {
        Ok(s) => s,
        Err(e) => panic!("response body must be valid UTF-8: {e}"),
    }
}

fn extract_challenge_from_bootstrap(html: &str) -> String {
    let marker = "var CHALLENGE = ";
    let Some(marker_pos) = html.find(marker) else {
        panic!("CHALLENGE marker missing");
    };
    let start = marker_pos + marker.len();
    let Some(semi_rel) = html[start..].find(';') else {
        panic!("CHALLENGE semicolon missing");
    };
    let end = start + semi_rel;
    match serde_json::from_str(&html[start..end]) {
        Ok(s) => s,
        Err(e) => panic!("CHALLENGE must be valid JSON string: {e}"),
    }
}

async fn authenticate(router: &Router, cipher: &AesGcmSessionCipher) -> String {
    let auth_request = match HttpRequest::builder().uri(AUTH_PATH).body(Body::empty()) {
        Ok(r) => r,
        Err(e) => panic!("build auth request: {e}"),
    };
    let auth_response = match router.clone().oneshot(auth_request).await {
        Ok(r) => r,
        Err(e) => panic!("oneshot auth request: {e}"),
    };
    assert_eq!(auth_response.status(), StatusCode::OK);
    assert_eq!(
        auth_response
            .headers()
            .get("referrer-policy")
            .and_then(|v| v.to_str().ok()),
        Some("no-referrer")
    );
    let html = response_text(auth_response).await;
    let challenge = extract_challenge_from_bootstrap(&html);

    let verify_body = cipher.encrypt(
        &serde_json::json!({ "challenge": challenge }).to_string(),
        Some(&format!("REQ:{AUTH_VERIFY_PATH}")),
    );
    let verify_request = match HttpRequest::builder()
        .method(Method::POST)
        .uri(AUTH_VERIFY_PATH)
        .body(Body::from(verify_body))
    {
        Ok(r) => r,
        Err(e) => panic!("build verify request: {e}"),
    };
    let verify_response = match router.clone().oneshot(verify_request).await {
        Ok(r) => r,
        Err(e) => panic!("oneshot verify request: {e}"),
    };
    assert_eq!(verify_response.status(), StatusCode::OK);

    let Some(set_cookie) = verify_response
        .headers()
        .get(SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
    else {
        panic!("Set-Cookie missing");
    };
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Lax"));
    assert!(set_cookie.contains("Max-Age=315360000"));

    let enc_resp = response_text(verify_response).await;
    let Some(plain_resp) = cipher.decrypt(&enc_resp, Some(&format!("RES:{AUTH_VERIFY_PATH}")))
    else {
        panic!("verify response must decrypt");
    };
    let parsed: Value = match serde_json::from_str(&plain_resp) {
        Ok(v) => v,
        Err(e) => panic!("parse verify response JSON: {e}"),
    };
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["challenge"], challenge);

    let Some(pair) = set_cookie.split(';').next() else {
        panic!("cookie pair missing");
    };
    pair.to_string()
}

async fn call_encrypted_api_raw(
    router: &Router,
    cipher: &AesGcmSessionCipher,
    cookie: &str,
    path: &str,
    payload: &Value,
) -> (ApiResultPayload, Value) {
    let body = cipher.encrypt(&payload.to_string(), Some(&format!("REQ:{path}")));
    let req = match HttpRequest::builder()
        .method(Method::POST)
        .uri(path)
        .header(COOKIE, cookie)
        .body(Body::from(body))
    {
        Ok(r) => r,
        Err(e) => panic!("build API request: {e}"),
    };
    let res = match router.clone().oneshot(req).await {
        Ok(r) => r,
        Err(e) => panic!("oneshot API request: {e}"),
    };
    assert_eq!(res.status(), StatusCode::OK);
    let enc_text = response_text(res).await;
    let Some(plain) = cipher.decrypt(&enc_text, Some(&format!("RES:{path}"))) else {
        panic!("API response must decrypt with RES:<path> AAD");
    };
    let raw_val: Value = match serde_json::from_str(&plain) {
        Ok(v) => v,
        Err(e) => panic!("parse API raw JSON: {e}"),
    };
    assert!(
        raw_val.get("msg").is_none(),
        "API wire response must carry code only, not localized msg: {plain}"
    );
    let parsed: ApiResultPayload = match serde_json::from_str(&plain) {
        Ok(p) => p,
        Err(e) => panic!("parse ApiResultPayload: {e}"),
    };
    (parsed, raw_val)
}

async fn call_encrypted_api(
    router: &Router,
    cipher: &AesGcmSessionCipher,
    cookie: &str,
    path: &str,
    payload: &Value,
) -> ApiResultPayload {
    call_encrypted_api_raw(router, cipher, cookie, path, payload)
        .await
        .0
}

#[tokio::test]
async fn static_routes_redirects_compression_and_fallback() {
    let (bridge_port, _rec) = spawn_mock_bridge().await;
    let (router, _state, cipher) = build_test_app(bridge_port, true);

    // 1. /favicon.ico
    let fav = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri(paths::FAVICON)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fav.status(), StatusCode::OK);
    assert_eq!(fav.headers().get(CONTENT_TYPE).unwrap(), "image/x-icon");
    assert_eq!(
        fav.headers().get(CACHE_CONTROL).unwrap(),
        "public, max-age=86400"
    );
    let fav_bytes = fav.into_body().collect().await.unwrap().to_bytes();
    assert!(!fav_bytes.is_empty());

    // 2. Unauthenticated GET / -> 302 /dh-console/un-auth
    let root_unauth = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri(paths::ROOT)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root_unauth.status(), StatusCode::FOUND);
    assert_eq!(root_unauth.headers().get(LOCATION).unwrap(), paths::UN_AUTH);

    // 3. Unauthenticated GET /dh-console/home_page -> 302 /dh-console/un-auth
    let home_unauth = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri(paths::HOME_PAGE)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(home_unauth.status(), StatusCode::FOUND);
    assert_eq!(home_unauth.headers().get(LOCATION).unwrap(), paths::UN_AUTH);

    // 4. GET /dh-console/un-auth with br / gzip / raw
    let unauth_br = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri(paths::UN_AUTH)
                .header(ACCEPT_ENCODING, "gzip, deflate, br")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth_br.status(), StatusCode::OK);
    assert_eq!(unauth_br.headers().get(CONTENT_ENCODING).unwrap(), "br");

    let unauth_gz = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri(paths::UN_AUTH)
                .header(ACCEPT_ENCODING, "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth_gz.status(), StatusCode::OK);
    assert_eq!(unauth_gz.headers().get(CONTENT_ENCODING).unwrap(), "gzip");

    // 5. Fallback: unknown GET -> 302, unknown POST -> 401, wrong method -> 401 / 302
    let unk_get = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri("/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unk_get.status(), StatusCode::FOUND);
    assert_eq!(unk_get.headers().get(LOCATION).unwrap(), paths::UN_AUTH);

    let unk_post = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .method(Method::POST)
                .uri("/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unk_post.status(), StatusCode::UNAUTHORIZED);

    let wrong_method_post = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .method(Method::POST)
                .uri(paths::HOME_PAGE)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_method_post.status(), StatusCode::UNAUTHORIZED);

    // 6. Authenticated GET / -> 302 /dh-console/home_page, GET /dh-console/home_page -> 200 with dh_cipher & lang inlined
    let cookie = authenticate(&router, &cipher).await;
    let root_auth = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri(paths::ROOT)
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root_auth.status(), StatusCode::FOUND);
    assert_eq!(root_auth.headers().get(LOCATION).unwrap(), paths::HOME_PAGE);

    let home_auth = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri(paths::HOME_PAGE)
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(home_auth.status(), StatusCode::OK);
    assert_eq!(home_auth.headers().get(CACHE_CONTROL).unwrap(), "no-store");
    let home_html = response_text(home_auth).await;
    assert!(home_html.contains("globalScope.__dhCipher"));
    assert!(!home_html.contains("/*__DH_CIPHER_INLINE__*/"));
    assert!(home_html.contains("window.__dhLang = "));
    assert!(!home_html.contains("/*__DH_LANG_INLINE__*/"));

    let unauth_raw = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri(paths::UN_AUTH)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth_raw.status(), StatusCode::OK);
    let unauth_html = response_text(unauth_raw).await;
    assert!(unauth_html.contains("window.__dhLang = "));
    assert!(!unauth_html.contains("/*__DH_LANG_INLINE__*/"));
}

#[tokio::test]
async fn challenge_replay_eviction_tamper_and_body_cap() {
    let (bridge_port, _rec) = spawn_mock_bridge().await;
    let (router, state, cipher) = build_test_app(bridge_port, false);

    // Challenge replay defense: second verify with same challenge must return 401
    let auth_res = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri(AUTH_PATH)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let challenge = extract_challenge_from_bootstrap(&response_text(auth_res).await);
    let body1 = cipher.encrypt(
        &serde_json::json!({ "challenge": challenge }).to_string(),
        Some(&format!("REQ:{AUTH_VERIFY_PATH}")),
    );
    let first = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .method(Method::POST)
                .uri(AUTH_VERIFY_PATH)
                .body(Body::from(body1.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let cookie = first
        .headers()
        .get(SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    let replay = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .method(Method::POST)
                .uri(AUTH_VERIFY_PATH)
                .body(Body::from(body1))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);

    // Eviction beyond MAX_PENDING_CHALLENGES (256)
    let oldest = state.auth.issue_challenge();
    for _ in 0..MAX_PENDING_CHALLENGES {
        let _ = state.auth.issue_challenge();
    }
    assert!(!state.auth.consume_challenge(&oldest));

    // Wrong PSK on verify -> 401
    let other_cipher = AesGcmSessionCipher::generate();
    let bad_verify_body = other_cipher.encrypt(
        r#"{"challenge":"abc"}"#,
        Some(&format!("REQ:{AUTH_VERIFY_PATH}")),
    );
    let bad_verify = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .method(Method::POST)
                .uri(AUTH_VERIFY_PATH)
                .body(Body::from(bad_verify_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_verify.status(), StatusCode::UNAUTHORIZED);

    // Missing cookie on encrypted POST -> 401
    let no_cookie = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .method(Method::POST)
                .uri(paths::KICK_PLAYER)
                .body(Body::from("anything"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(no_cookie.status(), StatusCode::UNAUTHORIZED);

    // Tampered cookie -> 401
    let tampered_cookie = format!(
        "{COOKIE_NAME}={}",
        other_cipher.encrypt("fake", Some(COOKIE_AAD))
    );
    let bad_cookie_res = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .method(Method::POST)
                .uri(paths::KICK_PLAYER)
                .header(COOKIE, tampered_cookie)
                .body(Body::from("anything"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_cookie_res.status(), StatusCode::UNAUTHORIZED);

    // Tampered cipher body / wrong AAD -> 401
    let wrong_aad_body = cipher.encrypt(
        r#"{"player_name":"Captain"}"#,
        Some("REQ:/dh-console/console_command"),
    );
    let wrong_aad_res = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .method(Method::POST)
                .uri(paths::KICK_PLAYER)
                .header(COOKIE, &cookie)
                .body(Body::from(wrong_aad_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_aad_res.status(), StatusCode::UNAUTHORIZED);

    // Body > 65,536 bytes -> 400 Bad Request
    let oversized = "A".repeat(MAX_ENCRYPTED_BODY_BYTES + 1);
    let too_big = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .method(Method::POST)
                .uri(paths::KICK_PLAYER)
                .header(COOKIE, &cookie)
                .body(Body::from(oversized))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(too_big.status(), StatusCode::BAD_REQUEST);

    // Valid ciphertext containing non-object JSON -> encrypted {status: 400, code: "API_NETWORK_PARAM_ERROR"}
    let array_body = cipher.encrypt("[1,2,3]", Some("REQ:/dh-console/kick_player"));
    let array_res = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .method(Method::POST)
                .uri(paths::KICK_PLAYER)
                .header(COOKIE, &cookie)
                .body(Body::from(array_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(array_res.status(), StatusCode::OK);
    let decrypted = cipher
        .decrypt(
            &response_text(array_res).await,
            Some("RES:/dh-console/kick_player"),
        )
        .unwrap();
    let parsed: ApiResultPayload = serde_json::from_str(&decrypted).unwrap();
    assert_eq!(parsed.status, status_code::BAD_REQUEST);
    assert_eq!(parsed.code, ApiCode::NetworkParamError);
}

#[tokio::test]
async fn all_seven_encrypted_api_endpoints_and_special_behaviors() {
    let (bridge_port, received) = spawn_mock_bridge().await;
    let (router, _state, cipher) = build_test_app(bridge_port, true);
    let cookie = authenticate(&router, &cipher).await;

    // 1. /dh-console/console_command with setisthrall 1 -> adds bone dagger + broadcasts to 7 others
    let (res, raw_res) = call_encrypted_api_raw(
        &router,
        &cipher,
        &cookie,
        paths::CONSOLE_COMMAND,
        &serde_json::json!({ "player_name": "Captain", "command": "setisthrall 1" }),
    )
    .await;
    assert_eq!(res.status, status_code::SUCCESS);
    assert_eq!(res.code, ApiCode::Success);
    assert_eq!(raw_res["code"], "API_SUCCESS");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    {
        let pkts = received.lock().unwrap();
        assert_eq!(pkts.len(), 1 + 1 + 7);
        assert_eq!(pkts[0]["action"], "execute_console_command");
        assert_eq!(pkts[1]["action"], "add_inventory_item");
        assert_eq!(pkts[1]["itemBlueprintPath"], BONE_DAGGER_ITEM);
        assert_eq!(pkts[1]["itemCount"], 1);
        assert!(pkts[2..].iter().all(|p| {
            p["action"] == "send_thrall_message"
                && p["targetRole"] != "Captain"
                && p["message"] == "船長 が闇の傀儡に悪墜ちしました"
        }));
    }
    received.lock().unwrap().clear();

    // 1b. setisthrall 0 -> does NOT add bone dagger, broadcasts unset
    let res_unset = call_encrypted_api(
        &router,
        &cipher,
        &cookie,
        paths::CONSOLE_COMMAND,
        &serde_json::json!({ "player_name": "Captain", "command": "setisthrall 0" }),
    )
    .await;
    assert_eq!(res_unset.status, status_code::SUCCESS);
    assert_eq!(res_unset.code, ApiCode::Success);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    {
        let pkts = received.lock().unwrap();
        assert_eq!(pkts.len(), 1 + 7);
        assert!(pkts.iter().all(|p| p["action"] != "add_inventory_item"));
    }
    received.lock().unwrap().clear();

    // 1c. mixed-case cmd_name -> normalized to lowercase in bridge packet & still triggers broadcast
    let res_blizz = call_encrypted_api(
        &router,
        &cipher,
        &cookie,
        paths::CONSOLE_COMMAND,
        &serde_json::json!({ "player_name": "Doctor", "command": "SetBlizzardIntensity 1.00" }),
    )
    .await;
    assert_eq!(res_blizz.status, status_code::SUCCESS);
    assert_eq!(res_blizz.code, ApiCode::Success);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    {
        let pkts = received.lock().unwrap();
        assert_eq!(pkts.len(), 1 + 7);
        assert_eq!(pkts[0]["action"], "execute_console_command");
        assert_eq!(pkts[0]["command"], "setblizzardintensity 1.00");
        assert!(pkts[1..].iter().all(|p| {
            p["action"] == "send_thrall_message"
                && p["targetRole"] != "Doctor"
                && p["message"] == "医者 が暴風雪を呼びました"
        }));
    }
    received.lock().unwrap().clear();

    // 2. /dh-console/set_human -> <=500 success, >500 highValueWarning, negative -> 416
    let res_stat_ok = call_encrypted_api(
        &router,
        &cipher,
        &cookie,
        paths::SET_HUMAN,
        &serde_json::json!({ "player_name": "Doctor", "prop_name": "Health", "value": "500" }),
    )
    .await;
    assert_eq!(res_stat_ok.status, status_code::SUCCESS);
    assert_eq!(res_stat_ok.code, ApiCode::Success);

    let (res_stat_high, raw_stat_high) = call_encrypted_api_raw(
        &router,
        &cipher,
        &cookie,
        paths::SET_HUMAN,
        &serde_json::json!({ "player_name": "Doctor", "prop_name": "Health", "value": 500.5 }),
    )
    .await;
    assert_eq!(res_stat_high.status, status_code::SUCCESS);
    assert_eq!(res_stat_high.code, ApiCode::HighValueWarning);
    assert_eq!(raw_stat_high["code"], "API_HIGH_VALUE_WARNING");

    let (res_stat_bad, raw_stat_bad) = call_encrypted_api_raw(
        &router,
        &cipher,
        &cookie,
        paths::SET_HUMAN,
        &serde_json::json!({ "player_name": "Doctor", "prop_name": "Health", "value": -5 }),
    )
    .await;
    assert_eq!(res_stat_bad.status, status_code::INVALID_VALUE);
    assert_eq!(res_stat_bad.code, ApiCode::InvalidValueError);
    assert_eq!(raw_stat_bad["code"], "API_INVALID_VALUE_ERROR");

    let res_item_bad = call_encrypted_api(
        &router,
        &cipher,
        &cookie,
        paths::ADD_ITEM,
        &serde_json::json!({ "player_name": "Cook", "item_name": "BP_Test", "count": 0 }),
    )
    .await;
    assert_eq!(res_item_bad.status, status_code::INVALID_VALUE);
    assert_eq!(res_item_bad.code, ApiCode::InvalidValueError);

    let res_storage_bad = call_encrypted_api(
        &router,
        &cipher,
        &cookie,
        paths::SET_STORAGE,
        &serde_json::json!({ "player_name": "Engineer", "limit": -1 }),
    )
    .await;
    assert_eq!(res_storage_bad.status, status_code::INVALID_VALUE);
    assert_eq!(res_storage_bad.code, ApiCode::InvalidValueError);

    // Drain packets from the two successful set_human calls (each sends 1 + 7 = 8 packets)
    for _ in 0..40 {
        if received.lock().unwrap().len() >= 16 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(received.lock().unwrap().len(), 16);
    received.lock().unwrap().clear();

    // 3. /dh-console/add_item
    let res_item = call_encrypted_api(
        &router,
        &cipher,
        &cookie,
        paths::ADD_ITEM,
        &serde_json::json!({
            "player_name": "Cook",
            "item_name": "/Game/Blueprints/Inventory/Coal/BP_Coal_Inventory.BP_Coal_Inventory_C",
            "count": "3"
        }),
    )
    .await;
    assert_eq!(res_item.status, status_code::SUCCESS);
    assert_eq!(res_item.code, ApiCode::Success);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    {
        let pkts = received.lock().unwrap();
        assert_eq!(pkts.len(), 1 + 7);
        assert_eq!(pkts[0]["action"], "add_inventory_item");
        assert_eq!(pkts[0]["itemCount"], 3);
        assert_eq!(pkts[1]["message"], "料理人 が 石炭 3 を獲得しました");
    }
    received.lock().unwrap().clear();

    // 4. /dh-console/set_storage
    let res_storage = call_encrypted_api(
        &router,
        &cipher,
        &cookie,
        paths::SET_STORAGE,
        &serde_json::json!({ "player_name": "Engineer", "limit": 12 }),
    )
    .await;
    assert_eq!(res_storage.status, status_code::SUCCESS);
    assert_eq!(res_storage.code, ApiCode::Success);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    {
        let pkts = received.lock().unwrap();
        assert_eq!(pkts.len(), 1 + 7);
        assert_eq!(pkts[0]["action"], "set_storage_limit");
        assert_eq!(pkts[0]["storageLimit"], 12);
    }
    received.lock().unwrap().clear();

    // 5. /dh-console/send_thrall_msg (no auto-broadcast)
    let res_msg = call_encrypted_api(
        &router,
        &cipher,
        &cookie,
        paths::SEND_THRALL_MSG,
        &serde_json::json!({ "player_name": "Hunter", "msg": "秘密メッセージ" }),
    )
    .await;
    assert_eq!(res_msg.status, status_code::SUCCESS);
    assert_eq!(res_msg.code, ApiCode::Success);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    {
        let pkts = received.lock().unwrap();
        assert_eq!(pkts.len(), 1);
        assert_eq!(pkts[0]["action"], "send_thrall_message");
        assert_eq!(pkts[0]["targetRole"], "Hunter");
        assert_eq!(pkts[0]["message"], "秘密メッセージ");
    }
    received.lock().unwrap().clear();

    // 6. /dh-console/send_thrall_msg/broadcast -> 8 roles
    let res_bcast = call_encrypted_api(
        &router,
        &cipher,
        &cookie,
        paths::BROADCAST,
        &serde_json::json!({ "msg": "全員へのお知らせ" }),
    )
    .await;
    assert_eq!(res_bcast.status, status_code::SUCCESS);
    assert_eq!(res_bcast.code, ApiCode::Success);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    {
        let pkts = received.lock().unwrap();
        assert_eq!(pkts.len(), 8);
        assert!(pkts.iter().all(|p| p["message"] == "全員へのお知らせ"));
    }
    received.lock().unwrap().clear();

    // 7. /dh-console/kick_player -> kick + 7 broadcasts
    let res_kick = call_encrypted_api(
        &router,
        &cipher,
        &cookie,
        paths::KICK_PLAYER,
        &serde_json::json!({ "player_name": "Navigator" }),
    )
    .await;
    assert_eq!(res_kick.status, status_code::SUCCESS);
    assert_eq!(res_kick.code, ApiCode::Success);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    {
        let pkts = received.lock().unwrap();
        assert_eq!(pkts.len(), 1 + 7);
        assert_eq!(pkts[0]["action"], "kick_player");
        assert_eq!(pkts[0]["targetRole"], "Navigator");
        assert_eq!(pkts[1]["message"], "航海士 がキックされました");
    }

    // 8. Unreachable bridge port returns encrypted { status: 504, code: "API_SERVER_EXECUTION_ERROR" }
    let unused_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let closed_port = unused_listener.local_addr().unwrap().port();
    drop(unused_listener);
    let (closed_router, _closed_state, closed_cipher) = build_test_app(closed_port, false);
    let closed_cookie = authenticate(&closed_router, &closed_cipher).await;
    let (res_504, raw_504) = call_encrypted_api_raw(
        &closed_router,
        &closed_cipher,
        &closed_cookie,
        paths::KICK_PLAYER,
        &serde_json::json!({ "player_name": "Captain" }),
    )
    .await;
    assert_eq!(res_504.status, status_code::SERVER_ERROR);
    assert_eq!(res_504.code, ApiCode::ServerExecutionError);
    assert_eq!(raw_504["code"], "API_SERVER_EXECUTION_ERROR");
}

#[tokio::test]
async fn start_web_console_binds_serves_and_shuts_down() {
    let logs = Arc::new(StdMutex::new(Vec::<String>::new()));
    let logs_clone = Arc::clone(&logs);
    let sink: WebLogSink = Arc::new(move |line| {
        logs_clone.lock().unwrap().push(line);
    });

    let handle = start_web_console(WebConsoleOptions {
        web_port: 0,
        console_patch_port: 17893,
        auto_broadcast: true,
        public_ip: Some("203.0.113.10".to_string()),
        log_sink: sink,
    })
    .await
    .unwrap();

    assert!(handle.port > 0);
    assert!(handle.display_info.public_url.starts_with(&format!(
        "http://203.0.113.10:{}/dh-console/auth#",
        handle.port
    )));

    // Probe real TCP HTTP server on GET /
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", handle.port))
        .await
        .unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut resp = String::new();
    stream.read_to_string(&mut resp).await.unwrap();
    assert!(resp.starts_with("HTTP/1.1 302"));
    assert!(resp.contains(paths::UN_AUTH));

    handle.shutdown().await;

    let captured = logs.lock().unwrap();
    assert!(captured.iter().any(|l| l.contains("Webコンソール待機開始")));
    assert!(captured
        .iter()
        .any(|l| l.contains("Webコンソールを停止しました")));
}

#[tokio::test]
async fn handles_accept_encoding_qvalues_malformed_cookies_and_huge_item_counts() {
    let (bridge_port, _rec) = spawn_mock_bridge().await;
    let (router, _state, cipher) = build_test_app(bridge_port, false);
    let cookie = authenticate(&router, &cipher).await;

    // br;q=0 must fall back to gzip; unrelated token containing "br" must serve uncompressed.
    let q0_res = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri(paths::UN_AUTH)
                .header(ACCEPT_ENCODING, "br;q=0, gzip;q=1.0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(q0_res.headers().get(CONTENT_ENCODING).unwrap(), "gzip");

    let notbr_res = router
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri(paths::UN_AUTH)
                .header(ACCEPT_ENCODING, "notbr")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(notbr_res.headers().get(CONTENT_ENCODING).is_none());

    // Malformed percent-escapes in cookies never panic and fall back to raw values.
    let cookies = parse_cookies(Some("a=1%; b=%ZZ; c=ok%20val; =empty"));
    assert_eq!(cookies.get("a").map(String::as_str), Some("1%"));
    assert_eq!(cookies.get("b").map(String::as_str), Some("%ZZ"));
    assert_eq!(cookies.get("c").map(String::as_str), Some("ok val"));

    // Huge digit string in add_item count is rejected cleanly without overflow or panic.
    let huge_digits = "9".repeat(5000);
    let res_huge = call_encrypted_api(
        &router,
        &cipher,
        &cookie,
        paths::ADD_ITEM,
        &serde_json::json!({ "player_name": "Cook", "item_name": "BP_Test", "count": huge_digits }),
    )
    .await;
    assert_eq!(res_huge.status, status_code::INVALID_VALUE);
    assert_eq!(res_huge.code, ApiCode::InvalidValueError);
}
