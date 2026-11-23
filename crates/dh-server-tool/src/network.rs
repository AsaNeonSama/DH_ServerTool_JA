use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub const PUBLIC_IP_HOST: &str = "checkip.amazonaws.com";
pub const PUBLIC_IP_PORT: u16 = 80;
pub const PUBLIC_IP_PATH: &str = "/";
pub const DEFAULT_PUBLIC_IP_TIMEOUT_MS: u64 = 3000;

const MAX_RESPONSE_BYTES: u64 = 8192;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicIpResult {
    pub ip: Option<String>,
    pub error: Option<String>,
}

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

fn decode_chunked_body(raw: &str) -> Option<String> {
    let mut out: String = String::new();
    let mut rest: &str = raw;
    loop {
        let (size_line, after_size): (&str, &str) = rest.split_once("\r\n")?;
        let size_hex: &str = size_line.split(';').next()?.trim();
        let chunk_len: usize = usize::from_str_radix(size_hex, 16).ok()?;
        if chunk_len == 0 {
            return Some(out);
        }
        if after_size.len() < chunk_len + 2 {
            return None;
        }
        out.push_str(&after_size[..chunk_len]);
        rest = &after_size[chunk_len + 2..];
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
                    && value.to_ascii_lowercase().contains("chunked")
            })
    });

    let decoded_body: String = if is_chunked {
        let Some(decoded): Option<String> = decode_chunked_body(raw_body) else {
            return PublicIpResult::err("Invalid chunked response");
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    async fn spawn_mock_server(response: &'static str, delay: Option<Duration>) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut req_buf = [0u8; 512];
                let _ = socket.read(&mut req_buf).await;
                if let Some(d) = delay {
                    tokio::time::sleep(d).await;
                }
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });
        port
    }

    #[tokio::test]
    async fn parses_valid_ipv4_and_ipv6_responses() {
        let port_v4 = spawn_mock_server(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n203.0.113.42\n",
            None,
        )
        .await;
        let res_v4 = fetch_public_ip_from("127.0.0.1", port_v4, "/", Duration::from_secs(2)).await;
        assert_eq!(res_v4.ip.as_deref(), Some("203.0.113.42"));
        assert_eq!(res_v4.error, None);

        let port_v6 = spawn_mock_server(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nb\r\n2001:db8::1\r\n0\r\n\r\n",
            None,
        )
        .await;
        let res_v6 = fetch_public_ip_from("127.0.0.1", port_v6, "/", Duration::from_secs(2)).await;
        assert_eq!(res_v6.ip.as_deref(), Some("2001:db8::1"));
        assert_eq!(res_v6.error, None);
    }

    #[tokio::test]
    async fn rejects_http_errors_invalid_body_and_timeouts() {
        let port_503 =
            spawn_mock_server("HTTP/1.1 503 Service Unavailable\r\n\r\nerror", None).await;
        let res_503 =
            fetch_public_ip_from("127.0.0.1", port_503, "/", Duration::from_secs(2)).await;
        assert_eq!(res_503.ip, None);
        assert_eq!(res_503.error.as_deref(), Some("HTTP 503"));

        let port_bad = spawn_mock_server("HTTP/1.1 200 OK\r\n\r\n<html>bad</html>", None).await;
        let res_bad =
            fetch_public_ip_from("127.0.0.1", port_bad, "/", Duration::from_secs(2)).await;
        assert_eq!(res_bad.ip, None);
        assert_eq!(res_bad.error.as_deref(), Some("Invalid IP response"));

        let port_slow = spawn_mock_server(
            "HTTP/1.1 200 OK\r\n\r\n1.2.3.4\n",
            Some(Duration::from_millis(500)),
        )
        .await;
        let res_slow =
            fetch_public_ip_from("127.0.0.1", port_slow, "/", Duration::from_millis(50)).await;
        assert_eq!(res_slow.ip, None);
        assert!(res_slow.error.is_some());
    }

    #[test]
    fn validates_ip_length_and_charset() {
        assert!(is_valid_public_ip_text("127.0.0.1"));
        assert!(is_valid_public_ip_text(
            "2001:0db8:85a3:0000:0000:8a2e:0370:7334"
        ));
        assert!(!is_valid_public_ip_text(""));
        assert!(!is_valid_public_ip_text(&"1".repeat(65)));
        assert!(!is_valid_public_ip_text("1.2.3.4 "));
        assert!(!is_valid_public_ip_text("1.2.3.4/24"));
    }
}
