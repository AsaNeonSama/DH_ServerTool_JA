use std::sync::{Arc, Mutex as StdMutex};

use dh_gadget_injector::log_channel::start_log_server;
use dh_gadget_injector::{InitHandshakeOutcome, InjectorEvent, LogLevel};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[tokio::test]
async fn log_channel_validates_nonce_and_streams_events() {
    let events: Arc<StdMutex<Vec<InjectorEvent>>> = Arc::new(StdMutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    let (server, init_rx) = start_log_server(Some(Arc::new(move |ev| {
        events_clone.lock().unwrap().push(ev);
    })))
    .await
    .expect("start_log_server");

    let port = server.port();
    let nonce = server.session_nonce().to_string();
    assert_eq!(nonce.len(), 32);

    // Unauthorized client with wrong nonce must be ignored
    {
        let mut bad = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        bad.write_all(b"{\"n\":\"deadbeef\",\"t\":\"INIT_OK\"}\n")
            .await
            .unwrap();
        let _ = bad.shutdown().await;
    }

    // Authorized client streams MOD_OK, LOG, INIT_OK
    let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let payload = format!(
        concat!(
            "{{\"n\":\"{n}\",\"t\":\"MOD_OK\",\"f\":\"console.js\"}}\n",
            "{{\"n\":\"{n}\",\"t\":\"LOG\",\"l\":\"WARN\",\"f\":\"network.js\",\"m\":\"日本語ログ\"}}\n",
            "{{\"n\":\"{n}\",\"t\":\"INIT_OK\"}}\n"
        ),
        n = nonce
    );
    client.write_all(payload.as_bytes()).await.unwrap();

    let outcome = tokio::time::timeout(std::time::Duration::from_secs(3), init_rx)
        .await
        .expect("init handshake timeout")
        .expect("init channel closed");
    assert_eq!(outcome, InitHandshakeOutcome::Ok);

    let captured = events.lock().unwrap().clone();
    assert!(captured.iter().any(|e| matches!(
        e,
        InjectorEvent::ModLoaded { file_name } if file_name == "console.js"
    )));
    assert!(captured.iter().any(|e| matches!(
        e,
        InjectorEvent::Log { level: LogLevel::Warn, file_name, message }
            if file_name == "network.js" && message == "日本語ログ"
    )));
    assert!(captured.iter().any(|e| matches!(e, InjectorEvent::InitOk)));

    drop(client);
    server.shutdown().await;
}

#[tokio::test]
async fn log_channel_propagates_init_err() {
    let (server, init_rx) = start_log_server(None).await.unwrap();
    let mut client = TcpStream::connect(("127.0.0.1", server.port()))
        .await
        .unwrap();
    let line = format!(
        "{{\"n\":\"{}\",\"t\":\"INIT_ERR\",\"f\":\"broken.js\",\"m\":\"ReferenceError\"}}\n",
        server.session_nonce()
    );
    client.write_all(line.as_bytes()).await.unwrap();

    let outcome = tokio::time::timeout(std::time::Duration::from_secs(3), init_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        outcome,
        InitHandshakeOutcome::Error {
            file_name: "broken.js".to_string(),
            message: "ReferenceError".to_string(),
        }
    );

    server.shutdown().await;
}
