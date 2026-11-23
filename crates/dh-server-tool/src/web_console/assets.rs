use axum::http::header::{CACHE_CONTROL, CONTENT_ENCODING, CONTENT_TYPE, VARY};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::crypto::AUTH_VERIFY_PATH;

pub struct CachedPage {
    pub raw: &'static [u8],
    pub gzip: &'static [u8],
    pub br: &'static [u8],
}

pub const CONTROL_PAGE_CACHE: CachedPage = CachedPage {
    raw: include_bytes!(concat!(env!("OUT_DIR"), "/dh_console.raw.html")),
    gzip: include_bytes!(concat!(env!("OUT_DIR"), "/dh_console.html.gz")),
    br: include_bytes!(concat!(env!("OUT_DIR"), "/dh_console.html.br")),
};

pub const UN_AUTH_PAGE_CACHE: CachedPage = CachedPage {
    raw: include_bytes!(concat!(env!("OUT_DIR"), "/un_auth.raw.html")),
    gzip: include_bytes!(concat!(env!("OUT_DIR"), "/un_auth.html.gz")),
    br: include_bytes!(concat!(env!("OUT_DIR"), "/un_auth.html.br")),
};

pub const FAVICON_BYTES: &[u8] = include_bytes!("../../../../assets/icon.ico");

pub const DH_CIPHER_JS: &str = include_str!("../../../../assets/console/dh_cipher.js");

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

#[must_use]
pub fn build_auth_bootstrap_html(challenge: &str) -> String {
    let challenge_json: String =
        serde_json::to_string(challenge).unwrap_or_else(|_: serde_json::Error| "\"\"".to_string());
    let verify_path_json: String = serde_json::to_string(AUTH_VERIFY_PATH)
        .unwrap_or_else(|_: serde_json::Error| "\"/dh-console/auth/verify\"".to_string());

    format!(
        r#"<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta name="referrer" content="no-referrer">
  <title>DH-Web-Console</title>
  <link rel="icon" type="image/x-icon" href="/favicon.ico">
  <style>
    :root {{
      --ink: #0b0b10;
      --paper: #f6f4ea;
      --cyan: #00e5ff;
      --yellow: #ffe600;
      --pink: #ff2a85;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      min-height: 100vh;
      min-height: 100dvh;
      display: flex;
      align-items: center;
      justify-content: center;
      background-color: var(--paper);
      background-image:
        linear-gradient(to right, rgba(11, 11, 16, 0.055) 1px, transparent 1px),
        linear-gradient(to bottom, rgba(11, 11, 16, 0.055) 1px, transparent 1px);
      background-size: 26px 26px;
    }}
    .dots {{
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 16px;
    }}
    .dots i {{
      display: block;
      width: 14px;
      height: 14px;
      border-radius: 50%;
      border: 2.5px solid var(--ink);
      animation: dotBounce 0.9s ease-in-out infinite;
    }}
    .dots i:nth-child(1) {{
      background-color: var(--cyan);
      animation-delay: 0s;
    }}
    .dots i:nth-child(2) {{
      background-color: var(--yellow);
      animation-delay: 0.16s;
    }}
    .dots i:nth-child(3) {{
      background-color: var(--pink);
      animation-delay: 0.32s;
    }}
    @keyframes dotBounce {{
      0%, 80%, 100% {{
        transform: translateY(0) scale(0.8);
        opacity: 0.55;
      }}
      40% {{
        transform: translateY(-8px) scale(1.22);
        opacity: 1;
      }}
    }}
  </style>
</head>
<body>
  <div class="dots" role="status" aria-label="Loading"><i></i><i></i><i></i></div>
  <script>
{DH_CIPHER_JS}
(async function () {{
  "use strict";
  var STORAGE_KEY = "dh_console_psk";
  var CHALLENGE = {challenge_json};
  var VERIFY_PATH = {verify_path_json};

  function fail() {{
    try {{ sessionStorage.removeItem(STORAGE_KEY); }} catch (_) {{}}
    try {{ localStorage.removeItem(STORAGE_KEY); }} catch (_) {{}}
    window.location.replace("/dh-console/un-auth");
  }}

  var psk = window.location.hash && window.location.hash.length > 1
    ? window.location.hash.slice(1).trim()
    : "";
  try {{
    window.history.replaceState(null, "", window.location.pathname);
  }} catch (_) {{}}

  if (!window.__dhCipher || !window.__dhCipher.isValidPsk(psk)) {{
    fail();
    return;
  }}

  try {{
    var cipherReq = window.__dhCipher.encrypt(
      JSON.stringify({{ challenge: CHALLENGE }}),
      psk,
      "REQ:" + VERIFY_PATH
    );
    var res = await window.fetch(VERIFY_PATH, {{
      method: "POST",
      credentials: "include",
      headers: {{ "Content-Type": "text/plain; charset=utf-8" }},
      body: cipherReq
    }});
    if (!res.ok) {{
      fail();
      return;
    }}
    var cipherRes = (await res.text()).trim();
    var plainRes = window.__dhCipher.decrypt(cipherRes, psk, "RES:" + VERIFY_PATH);
    if (plainRes === null) {{
      fail();
      return;
    }}
    var parsed = JSON.parse(plainRes);
    if (!parsed || parsed.ok !== true || parsed.challenge !== CHALLENGE) {{
      fail();
      return;
    }}
    try {{ sessionStorage.setItem(STORAGE_KEY, psk); }} catch (_) {{}}
    try {{ localStorage.setItem(STORAGE_KEY, psk); }} catch (_) {{}}
    window.location.replace("/dh-console/home_page");
  }} catch (_) {{
    fail();
  }}
}})();
  </script>
</body>
</html>"#
    )
}
