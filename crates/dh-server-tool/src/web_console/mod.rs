pub mod assets;
pub mod auth;
pub mod bridge;
pub mod logger;
pub mod routes;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::colors;
use crate::crypto::{AesGcmSessionCipher, AUTH_PATH, AUTH_PSK_SEPARATOR, AUTH_VERIFY_PATH};
use crate::messages::ja::terminal as terminal_msg;
use crate::messages::ja::web_console::{log_categories, log_details};
use crate::web_console::assets::{
    html_response, CONTROL_PAGE_CACHE, FAVICON_BYTES, UN_AUTH_PAGE_CACHE,
};
use crate::web_console::auth::{
    redirect_to, redirect_to_un_auth, ApiResultPayload, DecryptedPostRequest, WebConsoleAuth,
};
use crate::web_console::bridge::ConsoleBridge;
use crate::web_console::logger::{emit_web_log, WebLogSink};
use crate::web_console::routes::ApiRouteContext;

pub struct WebConsoleOptions {
    pub web_port: u16,
    pub console_patch_port: u16,
    pub auto_broadcast: bool,
    pub public_ip: Option<String>,
    pub log_sink: WebLogSink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebConsoleDisplayInfo {
    pub port: u16,
    pub console_patch_port: u16,
    pub auto_broadcast: bool,
    pub public_url: String,
    pub local_url: String,
}

pub struct WebConsoleState {
    pub auth: WebConsoleAuth,
    pub bridge: ConsoleBridge,
    pub auto_broadcast: bool,
    pub log_sink: WebLogSink,
}

impl WebConsoleState {
    #[must_use]
    pub fn new(
        auth_id: String,
        cipher: AesGcmSessionCipher,
        console_patch_port: u16,
        auto_broadcast: bool,
        log_sink: WebLogSink,
    ) -> Self {
        Self {
            auth: WebConsoleAuth::new(auth_id, cipher, Arc::clone(&log_sink)),
            bridge: ConsoleBridge::new(console_patch_port),
            auto_broadcast,
            log_sink,
        }
    }

    fn route_ctx(&self) -> ApiRouteContext<'_> {
        ApiRouteContext {
            bridge: &self.bridge,
            auto_broadcast: self.auto_broadcast,
            log_sink: &self.log_sink,
        }
    }
}

pub struct WebConsoleHandle {
    pub auth_id: String,
    pub auth_token: String,
    pub port: u16,
    pub display_info: WebConsoleDisplayInfo,
    shutdown_token: CancellationToken,
    log_sink: WebLogSink,
    stopped: Arc<AtomicBool>,
    task_handle: Option<JoinHandle<()>>,
}

impl WebConsoleHandle {
    pub fn stop(&self) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            emit_web_log(
                &self.log_sink,
                log_categories::SHUTDOWN,
                colors::LIGHT_YELLOW,
                log_details::SHUTDOWN_DETAIL,
            );
            self.shutdown_token.cancel();
        }
    }

    pub async fn shutdown(mut self) {
        self.stop();
        if let Some(handle) = self.task_handle.take() {
            let _: Result<(), tokio::task::JoinError> = handle.await;
        }
    }
}

impl Drop for WebConsoleHandle {
    fn drop(&mut self) {
        self.shutdown_token.cancel();
    }
}

fn peer_addr_from_request(req: &Request) -> Option<SocketAddr> {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci: &ConnectInfo<SocketAddr>| ci.0)
}

async fn favicon_handler() -> Response {
    let mut res: Response = (StatusCode::OK, FAVICON_BYTES).into_response();
    let headers: &mut HeaderMap = res.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("image/x-icon"));
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    res
}

async fn root_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    if state.auth.is_authenticated(req.headers()) {
        redirect_to("/dh-console/home_page")
    } else {
        redirect_to_un_auth()
    }
}

async fn un_auth_handler(req: Request) -> Response {
    html_response(req.headers(), &UN_AUTH_PAGE_CACHE, false)
}

async fn auth_handler(State(state): State<Arc<WebConsoleState>>) -> Response {
    state.auth.handle_auth()
}

async fn auth_verify_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    let peer: Option<SocketAddr> = peer_addr_from_request(&req);
    let (parts, body): (Parts, Body) = req.into_parts();
    state
        .auth
        .handle_auth_verify(&parts.headers, peer, body)
        .await
}

async fn home_page_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    if !state.auth.is_authenticated(req.headers()) {
        return redirect_to_un_auth();
    }
    html_response(req.headers(), &CONTROL_PAGE_CACHE, true)
}

async fn console_command_handler(
    State(state): State<Arc<WebConsoleState>>,
    req: Request,
) -> Response {
    const PATH: &str = "/dh-console/console_command";
    let peer: Option<SocketAddr> = peer_addr_from_request(&req);
    let (parts, body): (Parts, Body) = req.into_parts();
    match state
        .auth
        .prepare_encrypted_post(PATH, &parts.headers, peer, body)
        .await
    {
        DecryptedPostRequest::EarlyResponse(res) => res,
        DecryptedPostRequest::Ready { data, client_ip } => {
            let ctx: ApiRouteContext<'_> = state.route_ctx();
            let result: ApiResultPayload =
                routes::handle_console_command(&ctx, &data, &client_ip).await;
            state.auth.encrypted_api_response(PATH, &result)
        }
    }
}

async fn set_human_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    const PATH: &str = "/dh-console/set_human";
    let peer: Option<SocketAddr> = peer_addr_from_request(&req);
    let (parts, body): (Parts, Body) = req.into_parts();
    match state
        .auth
        .prepare_encrypted_post(PATH, &parts.headers, peer, body)
        .await
    {
        DecryptedPostRequest::EarlyResponse(res) => res,
        DecryptedPostRequest::Ready { data, client_ip } => {
            let ctx: ApiRouteContext<'_> = state.route_ctx();
            let result: ApiResultPayload = routes::handle_set_human(&ctx, &data, &client_ip).await;
            state.auth.encrypted_api_response(PATH, &result)
        }
    }
}

async fn add_item_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    const PATH: &str = "/dh-console/add_item";
    let peer: Option<SocketAddr> = peer_addr_from_request(&req);
    let (parts, body): (Parts, Body) = req.into_parts();
    match state
        .auth
        .prepare_encrypted_post(PATH, &parts.headers, peer, body)
        .await
    {
        DecryptedPostRequest::EarlyResponse(res) => res,
        DecryptedPostRequest::Ready { data, client_ip } => {
            let ctx: ApiRouteContext<'_> = state.route_ctx();
            let result: ApiResultPayload = routes::handle_add_item(&ctx, &data, &client_ip).await;
            state.auth.encrypted_api_response(PATH, &result)
        }
    }
}

async fn set_storage_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    const PATH: &str = "/dh-console/set_storage";
    let peer: Option<SocketAddr> = peer_addr_from_request(&req);
    let (parts, body): (Parts, Body) = req.into_parts();
    match state
        .auth
        .prepare_encrypted_post(PATH, &parts.headers, peer, body)
        .await
    {
        DecryptedPostRequest::EarlyResponse(res) => res,
        DecryptedPostRequest::Ready { data, client_ip } => {
            let ctx: ApiRouteContext<'_> = state.route_ctx();
            let result: ApiResultPayload =
                routes::handle_set_storage(&ctx, &data, &client_ip).await;
            state.auth.encrypted_api_response(PATH, &result)
        }
    }
}

async fn send_thrall_msg_handler(
    State(state): State<Arc<WebConsoleState>>,
    req: Request,
) -> Response {
    const PATH: &str = "/dh-console/send_thrall_msg";
    let peer: Option<SocketAddr> = peer_addr_from_request(&req);
    let (parts, body): (Parts, Body) = req.into_parts();
    match state
        .auth
        .prepare_encrypted_post(PATH, &parts.headers, peer, body)
        .await
    {
        DecryptedPostRequest::EarlyResponse(res) => res,
        DecryptedPostRequest::Ready { data, client_ip } => {
            let ctx: ApiRouteContext<'_> = state.route_ctx();
            let result: ApiResultPayload =
                routes::handle_send_thrall_msg(&ctx, &data, &client_ip).await;
            state.auth.encrypted_api_response(PATH, &result)
        }
    }
}

async fn broadcast_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    const PATH: &str = "/dh-console/send_thrall_msg/broadcast";
    let peer: Option<SocketAddr> = peer_addr_from_request(&req);
    let (parts, body): (Parts, Body) = req.into_parts();
    match state
        .auth
        .prepare_encrypted_post(PATH, &parts.headers, peer, body)
        .await
    {
        DecryptedPostRequest::EarlyResponse(res) => res,
        DecryptedPostRequest::Ready { data, client_ip } => {
            let ctx: ApiRouteContext<'_> = state.route_ctx();
            let result: ApiResultPayload = routes::handle_broadcast(&ctx, &data, &client_ip).await;
            state.auth.encrypted_api_response(PATH, &result)
        }
    }
}

async fn kick_player_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    const PATH: &str = "/dh-console/kick_player";
    let peer: Option<SocketAddr> = peer_addr_from_request(&req);
    let (parts, body): (Parts, Body) = req.into_parts();
    match state
        .auth
        .prepare_encrypted_post(PATH, &parts.headers, peer, body)
        .await
    {
        DecryptedPostRequest::EarlyResponse(res) => res,
        DecryptedPostRequest::Ready { data, client_ip } => {
            let ctx: ApiRouteContext<'_> = state.route_ctx();
            let result: ApiResultPayload =
                routes::handle_kick_player(&ctx, &data, &client_ip).await;
            state.auth.encrypted_api_response(PATH, &result)
        }
    }
}

async fn fallback_handler(method: Method) -> Response {
    if method != Method::GET {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    redirect_to_un_auth()
}

pub fn build_web_console_router(state: Arc<WebConsoleState>) -> Router {
    Router::new()
        .route("/favicon.ico", get(favicon_handler))
        .route("/", get(root_handler))
        .route("/dh-console/un-auth", get(un_auth_handler))
        .route(AUTH_PATH, get(auth_handler))
        .route(AUTH_VERIFY_PATH, post(auth_verify_handler))
        .route("/dh-console/home_page", get(home_page_handler))
        .route("/dh-console/console_command", post(console_command_handler))
        .route("/dh-console/set_human", post(set_human_handler))
        .route("/dh-console/add_item", post(add_item_handler))
        .route("/dh-console/set_storage", post(set_storage_handler))
        .route("/dh-console/send_thrall_msg", post(send_thrall_msg_handler))
        .route(
            "/dh-console/send_thrall_msg/broadcast",
            post(broadcast_handler),
        )
        .route("/dh-console/kick_player", post(kick_player_handler))
        .fallback(fallback_handler)
        .method_not_allowed_fallback(fallback_handler)
        // Body size cap (65,536 bytes) is enforced explicitly inside auth.rs so oversized
        // requests return 400 + structured Web Console logs matching the TS implementation.
        .layer(DefaultBodyLimit::disable())
        .with_state(state)
}

pub async fn start_web_console(options: WebConsoleOptions) -> std::io::Result<WebConsoleHandle> {
    let listener: TcpListener = TcpListener::bind(("0.0.0.0", options.web_port)).await?;
    let bound_port: u16 = listener.local_addr()?.port();

    let auth_id: String = Uuid::new_v4().to_string().to_uppercase();
    let cipher: AesGcmSessionCipher = AesGcmSessionCipher::generate();
    let auth_token: String = cipher.psk().to_string();

    let state: Arc<WebConsoleState> = Arc::new(WebConsoleState::new(
        auth_id.clone(),
        cipher,
        options.console_patch_port,
        options.auto_broadcast,
        Arc::clone(&options.log_sink),
    ));
    let router: Router = build_web_console_router(state);

    let display_public_ip: &str = options.public_ip.as_deref().unwrap_or("127.0.0.1");
    let auth_suffix: String = format!("{AUTH_PATH}{AUTH_PSK_SEPARATOR}{auth_token}");
    let display_info: WebConsoleDisplayInfo = WebConsoleDisplayInfo {
        port: bound_port,
        console_patch_port: options.console_patch_port,
        auto_broadcast: options.auto_broadcast,
        public_url: format!("http://{display_public_ip}:{bound_port}{auth_suffix}"),
        local_url: format!("http://127.0.0.1:{bound_port}{auth_suffix}"),
    };

    let broadcast_status_text: &str = if options.auto_broadcast {
        terminal_msg::TOGGLE_ON
    } else {
        terminal_msg::TOGGLE_OFF
    };
    emit_web_log(
        &options.log_sink,
        log_categories::STARTUP,
        colors::LIGHT_GREEN,
        &log_details::startup_detail(
            bound_port,
            options.console_patch_port,
            broadcast_status_text,
        ),
    );

    let shutdown_token: CancellationToken = CancellationToken::new();
    let server_token: CancellationToken = shutdown_token.clone();
    let task_handle: JoinHandle<()> = tokio::spawn(async move {
        let make_service: IntoMakeServiceWithConnectInfo<Router, SocketAddr> =
            router.into_make_service_with_connect_info::<SocketAddr>();
        let _: std::io::Result<()> = axum::serve(listener, make_service)
            .with_graceful_shutdown(server_token.cancelled_owned())
            .await;
    });

    Ok(WebConsoleHandle {
        auth_id,
        auth_token,
        port: bound_port,
        display_info,
        shutdown_token,
        log_sink: options.log_sink,
        stopped: Arc::new(AtomicBool::new(false)),
        task_handle: Some(task_handle),
    })
}

/// Helper for building non-TTY startup banner lines (exact 1:1 with `packages/webConsole/index.ts`).
#[must_use]
pub fn format_non_tty_startup_banner(info: &WebConsoleDisplayInfo) -> Vec<String> {
    let broadcast_text: &str = if info.auto_broadcast {
        terminal_msg::TOGGLE_ON
    } else {
        terminal_msg::TOGGLE_OFF
    };
    vec![
        String::new(),
        format!(
            "{} {}",
            colors::LIGHT_CYAN,
            terminal_msg::WEB_STARTED_HEADER
        ),
        format!(
            "{} * {} {}",
            colors::LIGHT_BLACK,
            terminal_msg::PUBLIC_URL_LABEL,
            info.public_url
        ),
        format!(
            "{} * {} {}",
            colors::LIGHT_BLACK,
            terminal_msg::LOCAL_URL_LABEL,
            info.local_url
        ),
        format!(
            "{} * {}{}{}{}{}",
            colors::LIGHT_YELLOW,
            terminal_msg::PSK_NOTICE_LEAD,
            colors::LIGHT_WHITE,
            terminal_msg::PSK_NOTICE_BODY,
            colors::LIGHT_MAGENTA,
            terminal_msg::PSK_NOTICE_WARN
        ),
        format!(
            "{} * {}",
            colors::LIGHT_BLACK,
            terminal_msg::internal_bridge_port_summary(info.console_patch_port, broadcast_text)
        ),
        format!(
            "{} {}",
            colors::LIGHT_CYAN,
            terminal_msg::WEB_STARTED_FOOTER
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    use axum::body::Body;
    use axum::http::header::{ACCEPT_ENCODING, CONTENT_ENCODING, COOKIE, LOCATION, SET_COOKIE};
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tower::ServiceExt;

    use crate::crypto::{COOKIE_AAD, MAX_PENDING_CHALLENGES};
    use crate::messages::ja::web_console::api;
    use crate::web_console::auth::{ApiResultPayload, COOKIE_NAME, MAX_ENCRYPTED_BODY_BYTES};
    use crate::web_console::bridge::BONE_DAGGER_ITEM;
    use crate::web_console::logger::noop_log_sink;
    use crate::web_console::routes::status_code;

    async fn spawn_mock_bridge() -> (u16, Arc<StdMutex<Vec<Value>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
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
                                rec.lock().unwrap().push(val);
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
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    fn extract_challenge_from_bootstrap(html: &str) -> String {
        let marker = "var CHALLENGE = ";
        let start = html.find(marker).expect("CHALLENGE marker missing") + marker.len();
        let end = html[start..]
            .find(';')
            .expect("CHALLENGE semicolon missing")
            + start;
        serde_json::from_str(&html[start..end]).expect("CHALLENGE must be valid JSON string")
    }

    async fn authenticate(router: &Router, cipher: &AesGcmSessionCipher) -> String {
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
        assert_eq!(auth_res.status(), StatusCode::OK);
        assert_eq!(
            auth_res
                .headers()
                .get("referrer-policy")
                .and_then(|v| v.to_str().ok()),
            Some("no-referrer")
        );
        let html = response_text(auth_res).await;
        let challenge = extract_challenge_from_bootstrap(&html);

        let verify_body = cipher.encrypt(
            &serde_json::json!({ "challenge": challenge }).to_string(),
            Some(&format!("REQ:{AUTH_VERIFY_PATH}")),
        );
        let verify_res = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method(Method::POST)
                    .uri(AUTH_VERIFY_PATH)
                    .body(Body::from(verify_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(verify_res.status(), StatusCode::OK);

        let set_cookie = verify_res
            .headers()
            .get(SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .expect("Set-Cookie missing")
            .to_string();
        assert!(set_cookie.contains("HttpOnly"));
        assert!(set_cookie.contains("SameSite=Lax"));
        assert!(set_cookie.contains("Max-Age=315360000"));

        let enc_resp = response_text(verify_res).await;
        let plain_resp = cipher
            .decrypt(&enc_resp, Some(&format!("RES:{AUTH_VERIFY_PATH}")))
            .expect("verify response must decrypt");
        let parsed: Value = serde_json::from_str(&plain_resp).unwrap();
        assert_eq!(parsed["ok"], true);
        assert_eq!(parsed["challenge"], challenge);

        set_cookie.split(';').next().unwrap().to_string()
    }

    async fn call_encrypted_api(
        router: &Router,
        cipher: &AesGcmSessionCipher,
        cookie: &str,
        path: &str,
        payload: &Value,
    ) -> ApiResultPayload {
        let body = cipher.encrypt(&payload.to_string(), Some(&format!("REQ:{path}")));
        let res = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header(COOKIE, cookie)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let enc_text = response_text(res).await;
        let plain = cipher
            .decrypt(&enc_text, Some(&format!("RES:{path}")))
            .expect("API response must decrypt with RES:<path> AAD");
        serde_json::from_str(&plain).unwrap()
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
                    .uri("/favicon.ico")
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
            .oneshot(HttpRequest::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(root_unauth.status(), StatusCode::FOUND);
        assert_eq!(
            root_unauth.headers().get(LOCATION).unwrap(),
            "/dh-console/un-auth"
        );

        // 3. Unauthenticated GET /dh-console/home_page -> 302 /dh-console/un-auth
        let home_unauth = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/dh-console/home_page")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(home_unauth.status(), StatusCode::FOUND);
        assert_eq!(
            home_unauth.headers().get(LOCATION).unwrap(),
            "/dh-console/un-auth"
        );

        // 4. GET /dh-console/un-auth with br / gzip / raw
        let unauth_br = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/dh-console/un-auth")
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
                    .uri("/dh-console/un-auth")
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
        assert_eq!(
            unk_get.headers().get(LOCATION).unwrap(),
            "/dh-console/un-auth"
        );

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
                    .uri("/dh-console/home_page")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(wrong_method_post.status(), StatusCode::UNAUTHORIZED);

        // 6. Authenticated GET / -> 302 /dh-console/home_page, GET /dh-console/home_page -> 200 with dh_cipher inlined
        let cookie = authenticate(&router, &cipher).await;
        let root_auth = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/")
                    .header(COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(root_auth.status(), StatusCode::FOUND);
        assert_eq!(
            root_auth.headers().get(LOCATION).unwrap(),
            "/dh-console/home_page"
        );

        let home_auth = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/dh-console/home_page")
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
                    .uri("/dh-console/kick_player")
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
                    .uri("/dh-console/kick_player")
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
                    .uri("/dh-console/kick_player")
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
                    .uri("/dh-console/kick_player")
                    .header(COOKIE, &cookie)
                    .body(Body::from(oversized))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(too_big.status(), StatusCode::BAD_REQUEST);

        // Valid ciphertext containing non-object JSON -> encrypted {status: 400}
        let array_body = cipher.encrypt("[1,2,3]", Some("REQ:/dh-console/kick_player"));
        let array_res = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method(Method::POST)
                    .uri("/dh-console/kick_player")
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
    }

    #[tokio::test]
    async fn all_seven_encrypted_api_endpoints_and_special_behaviors() {
        let (bridge_port, received) = spawn_mock_bridge().await;
        let (router, _state, cipher) = build_test_app(bridge_port, true);
        let cookie = authenticate(&router, &cipher).await;

        // 1. /dh-console/console_command with setisthrall 1 -> adds bone dagger + broadcasts to 7 others
        let res = call_encrypted_api(
            &router,
            &cipher,
            &cookie,
            "/dh-console/console_command",
            &serde_json::json!({ "player_name": "Captain", "command": "setisthrall 1" }),
        )
        .await;
        assert_eq!(res.status, status_code::SUCCESS);
        assert_eq!(res.msg, api::SUCCESS);
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
            "/dh-console/console_command",
            &serde_json::json!({ "player_name": "Captain", "command": "setisthrall 0" }),
        )
        .await;
        assert_eq!(res_unset.status, status_code::SUCCESS);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        {
            let pkts = received.lock().unwrap();
            assert_eq!(pkts.len(), 1 + 7);
            assert!(pkts.iter().all(|p| p["action"] != "add_inventory_item"));
        }
        received.lock().unwrap().clear();

        // 2. /dh-console/set_human -> <=500 success, >500 highValueWarning, negative -> 416
        let res_stat_ok = call_encrypted_api(
            &router,
            &cipher,
            &cookie,
            "/dh-console/set_human",
            &serde_json::json!({ "player_name": "Doctor", "prop_name": "Health", "value": "500" }),
        )
        .await;
        assert_eq!(res_stat_ok.status, status_code::SUCCESS);
        assert_eq!(res_stat_ok.msg, api::SUCCESS);

        let res_stat_high = call_encrypted_api(
            &router,
            &cipher,
            &cookie,
            "/dh-console/set_human",
            &serde_json::json!({ "player_name": "Doctor", "prop_name": "Health", "value": 500.5 }),
        )
        .await;
        assert_eq!(res_stat_high.status, status_code::SUCCESS);
        assert_eq!(res_stat_high.msg, api::HIGH_VALUE_WARNING);

        let res_stat_bad = call_encrypted_api(
            &router,
            &cipher,
            &cookie,
            "/dh-console/set_human",
            &serde_json::json!({ "player_name": "Doctor", "prop_name": "Health", "value": -5 }),
        )
        .await;
        assert_eq!(res_stat_bad.status, status_code::INVALID_VALUE);
        assert_eq!(res_stat_bad.msg, api::INVALID_VALUE_ERROR);

        let res_item_bad = call_encrypted_api(
            &router,
            &cipher,
            &cookie,
            "/dh-console/add_item",
            &serde_json::json!({ "player_name": "Cook", "item_name": "BP_Test", "count": 0 }),
        )
        .await;
        assert_eq!(res_item_bad.status, status_code::INVALID_VALUE);

        let res_storage_bad = call_encrypted_api(
            &router,
            &cipher,
            &cookie,
            "/dh-console/set_storage",
            &serde_json::json!({ "player_name": "Engineer", "limit": -1 }),
        )
        .await;
        assert_eq!(res_storage_bad.status, status_code::INVALID_VALUE);
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
            "/dh-console/add_item",
            &serde_json::json!({
                "player_name": "Cook",
                "item_name": "/Game/Blueprints/Inventory/Coal/BP_Coal_Inventory.BP_Coal_Inventory_C",
                "count": "3"
            }),
        )
        .await;
        assert_eq!(res_item.status, status_code::SUCCESS);
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
            "/dh-console/set_storage",
            &serde_json::json!({ "player_name": "Engineer", "limit": 12 }),
        )
        .await;
        assert_eq!(res_storage.status, status_code::SUCCESS);
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
            "/dh-console/send_thrall_msg",
            &serde_json::json!({ "player_name": "Hunter", "msg": "秘密メッセージ" }),
        )
        .await;
        assert_eq!(res_msg.status, status_code::SUCCESS);
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
            "/dh-console/send_thrall_msg/broadcast",
            &serde_json::json!({ "msg": "全員へのお知らせ" }),
        )
        .await;
        assert_eq!(res_bcast.status, status_code::SUCCESS);
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
            "/dh-console/kick_player",
            &serde_json::json!({ "player_name": "Navigator" }),
        )
        .await;
        assert_eq!(res_kick.status, status_code::SUCCESS);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        {
            let pkts = received.lock().unwrap();
            assert_eq!(pkts.len(), 1 + 7);
            assert_eq!(pkts[0]["action"], "kick_player");
            assert_eq!(pkts[0]["targetRole"], "Navigator");
            assert_eq!(pkts[1]["message"], "航海士 がキックされました");
        }

        // 8. Unreachable bridge port returns encrypted { status: 504, msg: SERVER_EXECUTION_ERROR }
        let unused_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let closed_port = unused_listener.local_addr().unwrap().port();
        drop(unused_listener);
        let (closed_router, _closed_state, closed_cipher) = build_test_app(closed_port, false);
        let closed_cookie = authenticate(&closed_router, &closed_cipher).await;
        let res_504 = call_encrypted_api(
            &closed_router,
            &closed_cipher,
            &closed_cookie,
            "/dh-console/kick_player",
            &serde_json::json!({ "player_name": "Captain" }),
        )
        .await;
        assert_eq!(res_504.status, status_code::SERVER_ERROR);
        assert_eq!(res_504.msg, api::SERVER_EXECUTION_ERROR);
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
        assert!(resp.contains("/dh-console/un-auth"));

        handle.shutdown().await;

        let captured = logs.lock().unwrap();
        assert!(captured.iter().any(|l| l.contains("Webコンソール待機開始")));
        assert!(captured
            .iter()
            .any(|l| l.contains("Webコンソールを停止しました")));
    }
}
