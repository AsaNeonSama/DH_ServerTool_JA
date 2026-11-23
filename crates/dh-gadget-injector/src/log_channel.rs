use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;

use rand::RngCore;
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader, Lines};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::error::Result;
use crate::{EventCallback, InjectorEvent, LogLevel};

#[derive(Debug, Deserialize)]
struct NdjsonPacket {
    n: String,
    t: String,
    #[serde(default)]
    l: Option<String>,
    #[serde(default)]
    f: Option<String>,
    #[serde(default)]
    m: Option<String>,
}

/// Initialization outcome reported by the Gadget bundle over the loopback channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitHandshakeOutcome {
    Ok,
    Error { file_name: String, message: String },
}

/// Active loopback NDJSON log & handshake TCP server (`127.0.0.1:0`).
#[derive(Debug)]
pub struct LogServerHandle {
    port: u16,
    session_nonce: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl LogServerHandle {
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    #[must_use]
    pub fn session_nonce(&self) -> &str {
        &self.session_nonce
    }

    /// Gracefully stops the accept loop and aborts the background reader task.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.task.take() {
            handle.abort();
            let _ = handle.await;
        }
    }
}

impl Drop for LogServerHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.task.take() {
            handle.abort();
        }
    }
}

fn generate_session_nonce() -> String {
    let mut bytes: [u8; 16] = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let mut hex: String = String::with_capacity(32);
    for b in bytes {
        let _ = write!(&mut hex, "{b:02x}");
    }
    hex
}

fn parse_log_level(raw: Option<&str>) -> LogLevel {
    match raw.unwrap_or("INFO") {
        "ERROR" | "error" => LogLevel::Error,
        "WARN" | "WARNING" | "warn" => LogLevel::Warn,
        _ => LogLevel::Info,
    }
}

/// Binds an ephemeral `127.0.0.1:0` NDJSON listener authenticated by a 128-bit
/// random `session_nonce`.
///
/// Returns the running [`LogServerHandle`] plus a oneshot receiver that resolves
/// when `INIT_OK` or `INIT_ERR` is received from the injected Gadget script.
pub async fn start_log_server(
    on_event: Option<EventCallback>,
) -> Result<(LogServerHandle, oneshot::Receiver<InitHandshakeOutcome>)> {
    let listener: TcpListener = TcpListener::bind("127.0.0.1:0").await?;
    let port: u16 = listener.local_addr()?.port();
    let session_nonce: String = generate_session_nonce();

    let (init_tx, init_rx): (
        oneshot::Sender<InitHandshakeOutcome>,
        oneshot::Receiver<InitHandshakeOutcome>,
    ) = oneshot::channel::<InitHandshakeOutcome>();
    let (shutdown_tx, mut shutdown_rx): (oneshot::Sender<()>, oneshot::Receiver<()>) =
        oneshot::channel::<()>();

    let shared_init_tx: Arc<Mutex<Option<oneshot::Sender<InitHandshakeOutcome>>>> =
        Arc::new(Mutex::new(Some(init_tx)));
    let expected_nonce: String = session_nonce.clone();

    let task: JoinHandle<()> = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    break;
                }
                accept_res = listener.accept() => {
                    let Ok((stream, _addr)): std::io::Result<(TcpStream, SocketAddr)> = accept_res else {
                        break;
                    };
                    let nonce: String = expected_nonce.clone();
                    let cb: Option<EventCallback> = on_event.clone();
                    let init_tx_slot: Arc<Mutex<Option<oneshot::Sender<InitHandshakeOutcome>>>> =
                        Arc::clone(&shared_init_tx);

                    tokio::spawn(async move {
                        let reader: BufReader<TcpStream> = BufReader::new(stream);
                        let mut lines: Lines<BufReader<TcpStream>> = reader.lines();
                        let mut authenticated: bool = false;

                        while let Ok(Some(line)) = lines.next_line().await {
                            let trimmed: &str = line.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            let Ok(packet): serde_json::Result<NdjsonPacket> =
                                serde_json::from_str::<NdjsonPacket>(trimmed)
                            else {
                                if !authenticated {
                                    // Drop unauthenticated garbage immediately.
                                    break;
                                }
                                continue;
                            };

                            if packet.n != nonce {
                                // Nonce mismatch: reject unknown local client.
                                break;
                            }
                            authenticated = true;

                            match packet.t.as_str() {
                                "MOD_OK" => {
                                    if let Some(ref f) = cb {
                                        f(InjectorEvent::ModLoaded {
                                            file_name: packet.f.unwrap_or_default(),
                                        });
                                    }
                                }
                                "LOG" => {
                                    if let Some(ref f) = cb {
                                        f(InjectorEvent::Log {
                                            level: parse_log_level(packet.l.as_deref()),
                                            file_name: packet
                                                .f
                                                .unwrap_or_else(|| "bundle".to_string()),
                                            message: packet.m.unwrap_or_default(),
                                        });
                                    }
                                }
                                "INIT_OK" => {
                                    if let Some(ref f) = cb {
                                        f(InjectorEvent::InitOk);
                                    }
                                    if let Some(tx) = init_tx_slot.lock().await.take() {
                                        let _ = tx.send(InitHandshakeOutcome::Ok);
                                    }
                                }
                                "INIT_ERR" => {
                                    let file_name: String =
                                        packet.f.unwrap_or_else(|| "bundle".to_string());
                                    let message: String = packet
                                        .m
                                        .unwrap_or_else(|| "unknown script init error".to_string());
                                    if let Some(ref f) = cb {
                                        f(InjectorEvent::InitError {
                                            file_name: file_name.clone(),
                                            message: message.clone(),
                                        });
                                    }
                                    if let Some(tx) = init_tx_slot.lock().await.take() {
                                        let _ = tx.send(InitHandshakeOutcome::Error {
                                            file_name,
                                            message,
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }

                        if authenticated {
                            if let Some(ref f) = cb {
                                f(InjectorEvent::ChannelClosed);
                            }
                        }
                    });
                }
            }
        }
    });

    Ok((
        LogServerHandle {
            port,
            session_nonce,
            shutdown_tx: Some(shutdown_tx),
            task: Some(task),
        },
        init_rx,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;
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
}
