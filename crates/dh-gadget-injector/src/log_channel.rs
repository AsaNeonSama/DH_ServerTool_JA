use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;

use rand::RngCore;
use tokio::io::{AsyncBufReadExt, BufReader, Lines};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::consts::log_channel::{
    PACKET_TYPE_INIT_ERR, PACKET_TYPE_INIT_OK, PACKET_TYPE_LOG, PACKET_TYPE_MOD_OK,
    SESSION_NONCE_BYTES,
};
use crate::error::Result;
use crate::structs::log_channel::NdjsonPacket;
pub use crate::structs::log_channel::{InitHandshakeOutcome, LogServerHandle};
use crate::{EventCallback, InjectorEvent, LogLevel};

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
    let mut bytes: [u8; SESSION_NONCE_BYTES] = [0u8; SESSION_NONCE_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let mut hex: String = String::with_capacity(SESSION_NONCE_BYTES * 2);
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
                                PACKET_TYPE_MOD_OK => {
                                    if let Some(ref f) = cb {
                                        f(InjectorEvent::ModLoaded {
                                            file_name: packet.f.unwrap_or_default(),
                                        });
                                    }
                                }
                                PACKET_TYPE_LOG => {
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
                                PACKET_TYPE_INIT_OK => {
                                    if let Some(ref f) = cb {
                                        f(InjectorEvent::InitOk);
                                    }
                                    if let Some(tx) = init_tx_slot.lock().await.take() {
                                        let _ = tx.send(InitHandshakeOutcome::Ok);
                                    }
                                }
                                PACKET_TYPE_INIT_ERR => {
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
