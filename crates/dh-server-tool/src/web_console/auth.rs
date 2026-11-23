use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE,
};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::colors;
use crate::crypto::{
    random_b64url_bytes, timing_safe_str_eq, AesGcmSessionCipher, AUTH_CHALLENGE_BYTES,
    AUTH_CHALLENGE_TTL_MS, AUTH_VERIFY_PATH, COOKIE_AAD, MAX_PENDING_CHALLENGES,
};
use crate::messages::ja::web_console::{api, log_categories, log_details};
use crate::web_console::assets::build_auth_bootstrap_html;
use crate::web_console::logger::{emit_web_log, normalize_client_ip, WebLogSink};
use crate::web_console::routes::status_code;

pub const COOKIE_NAME: &str = "dh_console_auth";
pub const COOKIE_MAX_AGE_SECONDS: u64 = 315_360_000;
pub const MAX_ENCRYPTED_BODY_BYTES: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiResultPayload {
    pub status: u16,
    pub msg: String,
}

#[must_use]
pub fn api_result(status: u16, msg: impl Into<String>) -> ApiResultPayload {
    ApiResultPayload {
        status,
        msg: msg.into(),
    }
}

#[must_use]
pub fn redirect_to(location: &'static str) -> Response {
    let mut res: Response = StatusCode::FOUND.into_response();
    res.headers_mut()
        .insert(LOCATION, HeaderValue::from_static(location));
    res
}

#[must_use]
pub fn redirect_to_un_auth() -> Response {
    redirect_to("/dh-console/un-auth")
}

fn decode_hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes: &[u8] = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i: usize = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return None;
            }
            let hi: u8 = decode_hex_nibble(bytes[i + 1])?;
            let lo: u8 = decode_hex_nibble(bytes[i + 2])?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[must_use]
pub fn parse_cookies(header: Option<&str>) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    let Some(raw): Option<&str> = header else {
        return out;
    };
    for part in raw.split(';') {
        let Some((k, v)): Option<(&str, &str)> = part.split_once('=') else {
            continue;
        };
        let key: &str = k.trim();
        let val: &str = v.trim();
        if key.is_empty() {
            continue;
        }
        let decoded: String = percent_decode(val).unwrap_or_else(|| val.to_string());
        out.insert(key.to_string(), decoded);
    }
    out
}

#[must_use]
pub fn resolve_client_ip(headers: &HeaderMap, peer: Option<SocketAddr>) -> String {
    if let Some(addr) = peer {
        let ip_text: String = addr.ip().to_string();
        return normalize_client_ip(Some(&ip_text));
    }
    let first_hop: Option<&str> = headers
        .get("x-forwarded-for")
        .and_then(|v: &HeaderValue| v.to_str().ok())
        .and_then(|s: &str| s.split(',').next());
    normalize_client_ip(first_hop)
}

pub async fn read_bounded_request_text(
    headers: &HeaderMap,
    body: Body,
    max_bytes: usize,
) -> Option<String> {
    if let Some(cl_val) = headers
        .get(CONTENT_LENGTH)
        .and_then(|v: &HeaderValue| v.to_str().ok())
    {
        if let Ok(declared) = cl_val.trim().parse::<usize>() {
            if declared > max_bytes {
                return None;
            }
        }
    }
    let bytes: Bytes = axum::body::to_bytes(body, max_bytes).await.ok()?;
    if bytes.len() > max_bytes {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).trim().to_string())
}

#[must_use]
pub fn parse_json_object_string(text: &str) -> Option<Map<String, Value>> {
    match serde_json::from_str::<Value>(text).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

pub enum DecryptedPostRequest {
    Ready {
        data: Map<String, Value>,
        client_ip: String,
    },
    EarlyResponse(Response),
}

pub struct WebConsoleAuth {
    auth_id: String,
    cipher: AesGcmSessionCipher,
    log_sink: WebLogSink,
    pending_challenges: Mutex<VecDeque<(String, Instant)>>,
}

impl WebConsoleAuth {
    #[must_use]
    pub fn new(auth_id: String, cipher: AesGcmSessionCipher, log_sink: WebLogSink) -> Self {
        Self {
            auth_id,
            cipher,
            log_sink,
            pending_challenges: Mutex::new(VecDeque::new()),
        }
    }

    #[must_use]
    pub fn cipher(&self) -> &AesGcmSessionCipher {
        &self.cipher
    }

    #[must_use]
    pub fn auth_id(&self) -> &str {
        &self.auth_id
    }

    #[must_use]
    pub fn issue_challenge(&self) -> String {
        let now: Instant = Instant::now();
        let mut queue: MutexGuard<'_, VecDeque<(String, Instant)>> = self
            .pending_challenges
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        queue.retain(|(_, expires_at): &(String, Instant)| *expires_at > now);
        while queue.len() >= MAX_PENDING_CHALLENGES {
            let _: Option<(String, Instant)> = queue.pop_front();
        }
        let challenge: String = random_b64url_bytes(AUTH_CHALLENGE_BYTES);
        queue.push_back((
            challenge.clone(),
            now + Duration::from_millis(AUTH_CHALLENGE_TTL_MS),
        ));
        challenge
    }

    #[must_use]
    pub fn consume_challenge(&self, challenge: &str) -> bool {
        let now: Instant = Instant::now();
        let mut queue: MutexGuard<'_, VecDeque<(String, Instant)>> = self
            .pending_challenges
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        queue.retain(|(_, expires_at): &(String, Instant)| *expires_at > now);
        if let Some(pos) = queue
            .iter()
            .position(|(stored, _): &(String, Instant)| timing_safe_str_eq(stored, challenge))
        {
            let _: Option<(String, Instant)> = queue.remove(pos);
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn is_authenticated(&self, headers: &HeaderMap) -> bool {
        let cookie_header: Option<&str> = headers
            .get(COOKIE)
            .and_then(|v: &HeaderValue| v.to_str().ok());
        let cookies: HashMap<String, String> = parse_cookies(cookie_header);
        let Some(session_cookie): Option<&String> = cookies.get(COOKIE_NAME) else {
            return false;
        };
        self.cipher
            .verify(session_cookie, &self.auth_id, Some(COOKIE_AAD))
    }

    fn encrypted_response<T: Serialize>(
        &self,
        pathname: &str,
        payload: &T,
        set_cookie_value: Option<&str>,
    ) -> Response {
        let Ok(plain_json): Result<String, serde_json::Error> = serde_json::to_string(payload)
        else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        let aad: String = format!("RES:{pathname}");
        let ciphertext: String = self.cipher.encrypt(&plain_json, Some(&aad));

        let mut res: Response = (StatusCode::OK, ciphertext).into_response();
        let headers: &mut HeaderMap = res.headers_mut();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if let Some(cookie_header) = set_cookie_value {
            if let Ok(val) = HeaderValue::from_str(cookie_header) {
                headers.insert(SET_COOKIE, val);
            }
        }
        res
    }

    #[must_use]
    pub fn encrypted_api_response(&self, pathname: &str, payload: &ApiResultPayload) -> Response {
        self.encrypted_response(pathname, payload, None)
    }

    #[must_use]
    pub fn handle_auth(&self) -> Response {
        let challenge: String = self.issue_challenge();
        let html: String = build_auth_bootstrap_html(&challenge);
        let mut res: Response = (StatusCode::OK, html).into_response();
        let headers: &mut HeaderMap = res.headers_mut();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        headers.insert(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("no-referrer"),
        );
        res
    }

    pub async fn handle_auth_verify(
        &self,
        headers: &HeaderMap,
        peer: Option<SocketAddr>,
        body: Body,
    ) -> Response {
        let client_ip: String = resolve_client_ip(headers, peer);
        let Some(raw_ciphertext): Option<String> =
            read_bounded_request_text(headers, body, MAX_ENCRYPTED_BODY_BYTES).await
        else {
            emit_web_log(
                &self.log_sink,
                log_categories::AUTH,
                colors::RED,
                &log_details::login_failed_invalid_body(&client_ip),
            );
            return StatusCode::BAD_REQUEST.into_response();
        };

        let req_aad: String = format!("REQ:{AUTH_VERIFY_PATH}");
        let Some(decrypted): Option<String> = self.cipher.decrypt(&raw_ciphertext, Some(&req_aad))
        else {
            emit_web_log(
                &self.log_sink,
                log_categories::AUTH,
                colors::RED,
                &log_details::login_failed_psk_mismatch(&client_ip),
            );
            return StatusCode::UNAUTHORIZED.into_response();
        };

        let challenge_opt: Option<String> =
            parse_json_object_string(&decrypted).and_then(|map: Map<String, Value>| {
                map.get("challenge")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });

        let Some(challenge): Option<String> =
            challenge_opt.filter(|c: &String| self.consume_challenge(c))
        else {
            emit_web_log(
                &self.log_sink,
                log_categories::AUTH,
                colors::LIGHT_YELLOW,
                &log_details::login_failed_challenge_invalid(&client_ip),
            );
            return StatusCode::UNAUTHORIZED.into_response();
        };

        let encrypted_cookie: String = self.cipher.encrypt(&self.auth_id, Some(COOKIE_AAD));
        emit_web_log(
            &self.log_sink,
            log_categories::AUTH,
            colors::LIGHT_GREEN,
            &log_details::login_succeeded(&client_ip),
        );

        let cookie_header: String = format!(
            "{COOKIE_NAME}={encrypted_cookie}; Path=/; HttpOnly; SameSite=Lax; Max-Age={COOKIE_MAX_AGE_SECONDS}"
        );
        let verify_payload: Value = serde_json::json!({
            "ok": true,
            "challenge": challenge,
        });
        self.encrypted_response(AUTH_VERIFY_PATH, &verify_payload, Some(&cookie_header))
    }

    pub async fn prepare_encrypted_post(
        &self,
        pathname: &str,
        headers: &HeaderMap,
        peer: Option<SocketAddr>,
        body: Body,
    ) -> DecryptedPostRequest {
        let client_ip: String = resolve_client_ip(headers, peer);
        if !self.is_authenticated(headers) {
            emit_web_log(
                &self.log_sink,
                log_categories::AUTH,
                colors::RED,
                &log_details::unauthenticated_rejected(&client_ip, pathname),
            );
            return DecryptedPostRequest::EarlyResponse(StatusCode::UNAUTHORIZED.into_response());
        }

        let Some(raw_ciphertext): Option<String> =
            read_bounded_request_text(headers, body, MAX_ENCRYPTED_BODY_BYTES).await
        else {
            emit_web_log(
                &self.log_sink,
                log_categories::ERROR,
                colors::RED,
                &log_details::invalid_request_body(&client_ip, pathname),
            );
            return DecryptedPostRequest::EarlyResponse(StatusCode::BAD_REQUEST.into_response());
        };

        let req_aad: String = format!("REQ:{pathname}");
        let Some(decrypted): Option<String> = self.cipher.decrypt(&raw_ciphertext, Some(&req_aad))
        else {
            emit_web_log(
                &self.log_sink,
                log_categories::AUTH,
                colors::RED,
                &log_details::decrypt_failed(&client_ip, pathname),
            );
            return DecryptedPostRequest::EarlyResponse(StatusCode::UNAUTHORIZED.into_response());
        };

        let Some(data): Option<Map<String, Value>> = parse_json_object_string(&decrypted) else {
            emit_web_log(
                &self.log_sink,
                log_categories::ERROR,
                colors::LIGHT_YELLOW,
                &log_details::invalid_json_params(&client_ip, pathname),
            );
            let err_payload: ApiResultPayload =
                api_result(status_code::BAD_REQUEST, api::NETWORK_PARAM_ERROR);
            return DecryptedPostRequest::EarlyResponse(
                self.encrypted_api_response(pathname, &err_payload),
            );
        };

        DecryptedPostRequest::Ready { data, client_ip }
    }
}
