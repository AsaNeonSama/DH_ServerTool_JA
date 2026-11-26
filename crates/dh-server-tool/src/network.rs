use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::consts::network::MAX_RESPONSE_BYTES;
pub use crate::consts::network::{
    DEFAULT_PUBLIC_IP_TIMEOUT_MS, PUBLIC_IP_HOST, PUBLIC_IP_PATH, PUBLIC_IP_PORT,
};
pub use crate::structs::network::PublicIpResult;

impl PublicIpResult {
    fn ok(ip: String) -> Self {
        Self {
            ip: Some(ip),
            error: None,
        }
    }

    fn err(message: impl Into<String>) -> Self {
        Self {
            ip: None,
            error: Some(message.into()),
        }
    }
}

#[must_use]
pub fn is_valid_public_ip_text(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .bytes()
            .all(|b: u8| b.is_ascii_hexdigit() || b == b'.' || b == b':')
}

fn find_crlf(bytes: &[u8]) -> Option<usize> {
    bytes.windows(2).position(|w: &[u8]| w == b"\r\n")
}

fn decode_chunked_body(raw: &[u8]) -> Option<Vec<u8>> {
    let max_bytes: usize = usize::try_from(MAX_RESPONSE_BYTES).unwrap_or(8192);
    let mut out: Vec<u8> = Vec::new();
    let mut rest: &[u8] = raw;
    loop {
        let line_end: usize = find_crlf(rest)?;
        let size_line_bytes: &[u8] = &rest[..line_end];
        let size_line: &str = std::str::from_utf8(size_line_bytes).ok()?;
        let size_hex: &str = size_line.split(';').next()?.trim();
        let chunk_len: usize = usize::from_str_radix(size_hex, 16).ok()?;
        let after_size: &[u8] = &rest[line_end + 2..];
        if chunk_len == 0 {
            return Some(out);
        }
        let chunk_with_crlf: usize = chunk_len.checked_add(2)?;
        if after_size.len() < chunk_with_crlf {
            return None;
        }
        if &after_size[chunk_len..chunk_with_crlf] != b"\r\n" {
            return None;
        }
        if out.len().checked_add(chunk_len)? > max_bytes {
            return None;
        }
        out.extend_from_slice(&after_size[..chunk_len]);
        rest = &after_size[chunk_with_crlf..];
    }
}

fn parse_http_ip_response(bytes: &[u8]) -> PublicIpResult {
    let Ok(response_text): Result<&str, std::str::Utf8Error> = std::str::from_utf8(bytes) else {
        return PublicIpResult::err("Invalid UTF-8 response");
    };

    let Some((header_block, raw_body)): Option<(&str, &str)> = response_text
        .split_once("\r\n\r\n")
        .or_else(|| response_text.split_once("\n\n"))
    else {
        return PublicIpResult::err("Malformed HTTP response");
    };

    let mut header_lines: std::str::Lines<'_> = header_block.lines();
    let Some(status_line): Option<&str> = header_lines.next() else {
        return PublicIpResult::err("Missing HTTP status line");
    };

    let status_code: Option<u16> = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s: &str| s.parse::<u16>().ok());
    let Some(code): Option<u16> = status_code else {
        return PublicIpResult::err("Invalid HTTP status line");
    };
    if !(200..300).contains(&code) {
        return PublicIpResult::err(format!("HTTP {code}"));
    }

    let is_chunked: bool = header_lines.any(|line: &str| {
        line.split_once(':')
            .is_some_and(|(name, value): (&str, &str)| {
                name.trim().eq_ignore_ascii_case("transfer-encoding")
                    && value
                        .split(',')
                        .any(|tok: &str| tok.trim().eq_ignore_ascii_case("chunked"))
            })
    });

    let decoded_body: String = if is_chunked {
        let Some(decoded_bytes): Option<Vec<u8>> = decode_chunked_body(raw_body.as_bytes()) else {
            return PublicIpResult::err("Invalid chunked response");
        };
        let Ok(decoded): Result<String, std::string::FromUtf8Error> =
            String::from_utf8(decoded_bytes)
        else {
            return PublicIpResult::err("Invalid UTF-8 response");
        };
        decoded
    } else {
        raw_body.to_string()
    };

    let trimmed: &str = decoded_body.trim();
    if !is_valid_public_ip_text(trimmed) {
        return PublicIpResult::err("Invalid IP response");
    }

    PublicIpResult::ok(trimmed.to_string())
}

pub async fn fetch_public_ip_from(
    host: &str,
    port: u16,
    path: &str,
    timeout: Duration,
) -> PublicIpResult {
    let io_result: Result<Result<Vec<u8>, String>, tokio::time::error::Elapsed> =
        tokio::time::timeout(timeout, async {
            let mut stream: TcpStream = TcpStream::connect((host, port))
                .await
                .map_err(|e: std::io::Error| e.to_string())?;
            let request: String = format!(
                "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: DH_ServerTool_JA/2.0\r\nConnection: close\r\n\r\n"
            );
            stream
                .write_all(request.as_bytes())
                .await
                .map_err(|e: std::io::Error| e.to_string())?;
            stream
                .flush()
                .await
                .map_err(|e: std::io::Error| e.to_string())?;

            let mut buf: Vec<u8> = Vec::with_capacity(256);
            let _: usize = stream
                .take(MAX_RESPONSE_BYTES)
                .read_to_end(&mut buf)
                .await
                .map_err(|e: std::io::Error| e.to_string())?;

            Ok::<Vec<u8>, String>(buf)
        })
        .await;

    match io_result {
        Ok(Ok(bytes)) => parse_http_ip_response(&bytes),
        Ok(Err(err)) => PublicIpResult::err(err),
        Err(_) => PublicIpResult::err("Request timed out"),
    }
}

pub async fn fetch_public_ip(timeout_ms: u64) -> PublicIpResult {
    fetch_public_ip_from(
        PUBLIC_IP_HOST,
        PUBLIC_IP_PORT,
        PUBLIC_IP_PATH,
        Duration::from_millis(timeout_ms),
    )
    .await
}
