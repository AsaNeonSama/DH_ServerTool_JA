use std::time::Duration;

use dh_server_tool::network::{fetch_public_ip_from, is_valid_public_ip_text};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn spawn_mock_server(response: &'static str, delay: Option<Duration>) -> u16 {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) => panic!("bind mock server: {e}"),
    };
    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(e) => panic!("mock server local_addr: {e}"),
    };
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
    let port_503 = spawn_mock_server("HTTP/1.1 503 Service Unavailable\r\n\r\nerror", None).await;
    let res_503 = fetch_public_ip_from("127.0.0.1", port_503, "/", Duration::from_secs(2)).await;
    assert_eq!(res_503.ip, None);
    assert_eq!(res_503.error.as_deref(), Some("HTTP 503"));

    let port_bad = spawn_mock_server("HTTP/1.1 200 OK\r\n\r\n<html>bad</html>", None).await;
    let res_bad = fetch_public_ip_from("127.0.0.1", port_bad, "/", Duration::from_secs(2)).await;
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
