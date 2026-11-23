use axum::http::header::{CACHE_CONTROL, CONTENT_ENCODING, CONTENT_TYPE, VARY};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::consts::web_console::{auth_bootstrap_tokens, paths};
pub use crate::consts::web_console::{
    AUTH_BOOTSTRAP_TEMPLATE, CONTROL_PAGE_CACHE, DH_CIPHER_JS, FAVICON_BYTES, UN_AUTH_PAGE_CACHE,
};
use crate::crypto::AUTH_VERIFY_PATH;
pub use crate::structs::web_console::CachedPage;

#[must_use]
pub fn html_response(headers: &HeaderMap, page: &CachedPage, no_store: bool) -> Response {
    let accept: &str = headers
        .get(axum::http::header::ACCEPT_ENCODING)
        .and_then(|v: &HeaderValue| v.to_str().ok())
        .unwrap_or("");

    let (body, encoding): (&'static [u8], Option<&'static str>) = if accept.contains("br") {
        (page.br, Some("br"))
    } else if accept.contains("gzip") {
        (page.gzip, Some("gzip"))
    } else {
        (page.raw, None)
    };

    let mut res: Response = (StatusCode::OK, body).into_response();
    let res_headers: &mut HeaderMap = res.headers_mut();
    res_headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    res_headers.insert(VARY, HeaderValue::from_static("Accept-Encoding"));
    if no_store {
        res_headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    if let Some(enc) = encoding {
        res_headers.insert(CONTENT_ENCODING, HeaderValue::from_static(enc));
    }
    res
}

fn json_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_: serde_json::Error| "\"\"".to_string())
}

#[must_use]
pub fn build_auth_bootstrap_html(challenge: &str) -> String {
    let challenge_json: String = json_string_literal(challenge);
    let verify_path_json: String = json_string_literal(AUTH_VERIFY_PATH);
    let un_auth_path_json: String = json_string_literal(paths::UN_AUTH);
    let home_page_path_json: String = json_string_literal(paths::HOME_PAGE);

    AUTH_BOOTSTRAP_TEMPLATE
        .replacen(auth_bootstrap_tokens::CHALLENGE, &challenge_json, 1)
        .replacen(auth_bootstrap_tokens::VERIFY_PATH, &verify_path_json, 1)
        .replacen(auth_bootstrap_tokens::UN_AUTH_PATH, &un_auth_path_json, 1)
        .replacen(
            auth_bootstrap_tokens::HOME_PAGE_PATH,
            &home_page_path_json,
            1,
        )
}
