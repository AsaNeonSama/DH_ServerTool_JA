pub mod assets;
pub mod auth;
pub mod bridge;
pub mod logger;
pub mod routes;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

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
use tokio::task::{AbortHandle, JoinHandle};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::colors;
use crate::consts::web_console::{paths, WEB_CONSOLE_SHUTDOWN_GRACE_MS};
use crate::crypto::{AesGcmSessionCipher, AUTH_PATH, AUTH_PSK_SEPARATOR, AUTH_VERIFY_PATH};
use crate::languages;
use crate::structs::languages::{TerminalMsg, TerminalMsgCode, WebLogEvent};
use crate::structs::web_console::EncryptedEndpoint;
pub use crate::structs::web_console::{
    WebConsoleDisplayInfo, WebConsoleHandle, WebConsoleOptions, WebConsoleState,
};
use crate::web_console::assets::{
    html_response, CONTROL_PAGE_CACHE, FAVICON_BYTES, UN_AUTH_PAGE_CACHE,
};
use crate::web_console::auth::{
    redirect_to, redirect_to_un_auth, ApiResultPayload, DecryptedPostRequest, WebConsoleAuth,
};
use crate::web_console::bridge::ConsoleBridge;
use crate::web_console::logger::{emit_web_log_event, WebLogSink};
use crate::web_console::routes::ApiRouteContext;

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

impl WebConsoleHandle {
    pub fn stop(&self) {
        if !self.stopped.swap(true, Ordering::SeqCst) {
            emit_web_log_event(&self.log_sink, &WebLogEvent::Shutdown);
            self.shutdown_token.cancel();
        }
    }

    pub async fn shutdown(mut self) {
        self.stop();
        if let Some(handle) = self.task_handle.take() {
            let abort_handle: AbortHandle = handle.abort_handle();
            if tokio::time::timeout(Duration::from_millis(WEB_CONSOLE_SHUTDOWN_GRACE_MS), handle)
                .await
                .is_err()
            {
                abort_handle.abort();
            }
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
        redirect_to(paths::HOME_PAGE)
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

impl EncryptedEndpoint {
    const fn path(self) -> &'static str {
        match self {
            Self::ConsoleCommand => paths::CONSOLE_COMMAND,
            Self::SetHuman => paths::SET_HUMAN,
            Self::AddItem => paths::ADD_ITEM,
            Self::SetStorage => paths::SET_STORAGE,
            Self::SendThrallMsg => paths::SEND_THRALL_MSG,
            Self::Broadcast => paths::BROADCAST,
            Self::KickPlayer => paths::KICK_PLAYER,
        }
    }
}

async fn dispatch_encrypted_post(
    state: Arc<WebConsoleState>,
    req: Request,
    endpoint: EncryptedEndpoint,
) -> Response {
    let pathname: &'static str = endpoint.path();
    let peer: Option<SocketAddr> = peer_addr_from_request(&req);
    let (parts, body): (Parts, Body) = req.into_parts();
    match state
        .auth
        .prepare_encrypted_post(pathname, &parts.headers, peer, body)
        .await
    {
        DecryptedPostRequest::EarlyResponse(res) => res,
        DecryptedPostRequest::Ready { data, client_ip } => {
            let ctx: ApiRouteContext<'_> = state.route_ctx();
            let result: ApiResultPayload = match endpoint {
                EncryptedEndpoint::ConsoleCommand => {
                    routes::handle_console_command(&ctx, &data, &client_ip).await
                }
                EncryptedEndpoint::SetHuman => {
                    routes::handle_set_human(&ctx, &data, &client_ip).await
                }
                EncryptedEndpoint::AddItem => {
                    routes::handle_add_item(&ctx, &data, &client_ip).await
                }
                EncryptedEndpoint::SetStorage => {
                    routes::handle_set_storage(&ctx, &data, &client_ip).await
                }
                EncryptedEndpoint::SendThrallMsg => {
                    routes::handle_send_thrall_msg(&ctx, &data, &client_ip).await
                }
                EncryptedEndpoint::Broadcast => {
                    routes::handle_broadcast(&ctx, &data, &client_ip).await
                }
                EncryptedEndpoint::KickPlayer => {
                    routes::handle_kick_player(&ctx, &data, &client_ip).await
                }
            };
            state.auth.encrypted_api_response(pathname, &result)
        }
    }
}

async fn console_command_handler(
    State(state): State<Arc<WebConsoleState>>,
    req: Request,
) -> Response {
    dispatch_encrypted_post(state, req, EncryptedEndpoint::ConsoleCommand).await
}

async fn set_human_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    dispatch_encrypted_post(state, req, EncryptedEndpoint::SetHuman).await
}

async fn add_item_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    dispatch_encrypted_post(state, req, EncryptedEndpoint::AddItem).await
}

async fn set_storage_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    dispatch_encrypted_post(state, req, EncryptedEndpoint::SetStorage).await
}

async fn send_thrall_msg_handler(
    State(state): State<Arc<WebConsoleState>>,
    req: Request,
) -> Response {
    dispatch_encrypted_post(state, req, EncryptedEndpoint::SendThrallMsg).await
}

async fn broadcast_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    dispatch_encrypted_post(state, req, EncryptedEndpoint::Broadcast).await
}

async fn kick_player_handler(State(state): State<Arc<WebConsoleState>>, req: Request) -> Response {
    dispatch_encrypted_post(state, req, EncryptedEndpoint::KickPlayer).await
}

async fn fallback_handler(method: Method) -> Response {
    if method != Method::GET {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    redirect_to_un_auth()
}

pub fn build_web_console_router(state: Arc<WebConsoleState>) -> Router {
    Router::new()
        .route(paths::FAVICON, get(favicon_handler))
        .route(paths::ROOT, get(root_handler))
        .route(paths::UN_AUTH, get(un_auth_handler))
        .route(AUTH_PATH, get(auth_handler))
        .route(AUTH_VERIFY_PATH, post(auth_verify_handler))
        .route(paths::HOME_PAGE, get(home_page_handler))
        .route(paths::CONSOLE_COMMAND, post(console_command_handler))
        .route(paths::SET_HUMAN, post(set_human_handler))
        .route(paths::ADD_ITEM, post(add_item_handler))
        .route(paths::SET_STORAGE, post(set_storage_handler))
        .route(paths::SEND_THRALL_MSG, post(send_thrall_msg_handler))
        .route(paths::BROADCAST, post(broadcast_handler))
        .route(paths::KICK_PLAYER, post(kick_player_handler))
        .fallback(fallback_handler)
        .method_not_allowed_fallback(fallback_handler)
        // Body size cap (65,536 bytes) is enforced explicitly inside auth.rs so oversized
        // requests return 400 + structured Web Console logs matching the protocol spec.
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

    emit_web_log_event(
        &options.log_sink,
        &WebLogEvent::Startup {
            web_port: bound_port,
            patch_port: options.console_patch_port,
            auto_broadcast: options.auto_broadcast,
        },
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

/// Helper for building non-TTY startup banner lines.
#[must_use]
pub fn format_non_tty_startup_banner(info: &WebConsoleDisplayInfo) -> Vec<String> {
    let bridge_summary: String =
        languages::render_terminal(&TerminalMsg::InternalBridgePortSummary {
            port: info.console_patch_port,
            auto_broadcast: info.auto_broadcast,
        });
    vec![
        String::new(),
        format!(
            "{} {}",
            colors::LIGHT_CYAN,
            languages::terminal_text(TerminalMsgCode::WebStartedHeader)
        ),
        format!(
            "{} * {} {}",
            colors::LIGHT_BLACK,
            languages::terminal_text(TerminalMsgCode::PublicUrlLabel),
            info.public_url
        ),
        format!(
            "{} * {} {}",
            colors::LIGHT_BLACK,
            languages::terminal_text(TerminalMsgCode::LocalUrlLabel),
            info.local_url
        ),
        format!(
            "{} * {}{}{}{}{}",
            colors::LIGHT_YELLOW,
            languages::terminal_text(TerminalMsgCode::PskNoticeLead),
            colors::LIGHT_WHITE,
            languages::terminal_text(TerminalMsgCode::PskNoticeBody),
            colors::LIGHT_MAGENTA,
            languages::terminal_text(TerminalMsgCode::PskNoticeWarn)
        ),
        format!("{} * {}", colors::LIGHT_BLACK, bridge_summary),
        format!(
            "{} {}",
            colors::LIGHT_CYAN,
            languages::terminal_text(TerminalMsgCode::WebStartedFooter)
        ),
    ]
}
