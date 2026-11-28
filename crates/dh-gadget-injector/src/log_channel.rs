use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use rand::rngs::{SysError, SysRng};
use rand::TryRng;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::task::{JoinHandle, JoinSet};

use crate::consts::log_channel::{
    MAX_CONCURRENT_LOG_CLIENTS, MAX_NDJSON_LINE_BYTES, PACKET_TYPE_INIT_ERR, PACKET_TYPE_INIT_OK,
    PACKET_TYPE_LOG, PACKET_TYPE_MOD_OK, SESSION_NONCE_BYTES, UNAUTH_FIRST_LINE_TIMEOUT_SECS,
};
use crate::error::Result;
use crate::structs::log_channel::{BoundedLine, NdjsonPacket};
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

    /// Gracefully stops the accept loop and aborts all active background reader tasks.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.task.take() {
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

fn generate_session_nonce() -> Result<String> {
    let mut bytes: [u8; SESSION_NONCE_BYTES] = [0u8; SESSION_NONCE_BYTES];
    SysRng.try_fill_bytes(&mut bytes).map_err(|e: SysError| {
        crate::error::InjectorError::Inject(format!("OS CSPRNG failed to generate nonce: {e}"))
    })?;
    let mut hex: String = String::with_capacity(SESSION_NONCE_BYTES * 2);
    for b in bytes {
        let _ = write!(&mut hex, "{b:02x}");
    }
    Ok(hex)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn parse_log_level(raw: Option<&str>) -> LogLevel {
    let level: &str = raw.unwrap_or("INFO");
    if level.eq_ignore_ascii_case("error") {
        LogLevel::Error
    } else if level.eq_ignore_ascii_case("warn") || level.eq_ignore_ascii_case("warning") {
        LogLevel::Warn
    } else {
        LogLevel::Info
    }
}

async fn read_bounded_line(
    reader: &mut BufReader<TcpStream>,
    max_bytes: usize,
    fail_fast_on_overflow: bool,
) -> std::io::Result<BoundedLine> {
    let mut buf: Vec<u8> = Vec::new();
    let mut overflowed: bool = false;
    let mut discarded: usize = 0;
    let max_discard: usize = max_bytes.saturating_mul(4);

    loop {
        let available: &[u8] = reader.fill_buf().await?;
        if available.is_empty() {
            if overflowed {
                return Ok(BoundedLine::TooLong);
            }
            if buf.is_empty() {
                return Ok(BoundedLine::Eof);
            }
            if buf.last() == Some(&b'\r') {
                let _: Option<u8> = buf.pop();
            }
            return Ok(BoundedLine::Line(buf));
        }

        if let Some(nl_pos) = available.iter().position(|&b: &u8| b == b'\n') {
            if !overflowed && buf.len().saturating_add(nl_pos) <= max_bytes {
                buf.extend_from_slice(&available[..nl_pos]);
                reader.consume(nl_pos + 1);
                if buf.last() == Some(&b'\r') {
                    let _: Option<u8> = buf.pop();
                }
                return Ok(BoundedLine::Line(buf));
            }
            buf.clear();
            reader.consume(nl_pos + 1);
            return Ok(BoundedLine::TooLong);
        }

        let chunk_len: usize = available.len();
        if overflowed {
            discarded = discarded.saturating_add(chunk_len);
        } else if buf.len().saturating_add(chunk_len) <= max_bytes {
            buf.extend_from_slice(available);
        } else {
            overflowed = true;
            buf.clear();
            discarded = discarded.saturating_add(chunk_len);
        }
        reader.consume(chunk_len);

        if overflowed && (fail_fast_on_overflow || discarded >= max_discard) {
            return Ok(BoundedLine::TooLong);
        }
    }
}

async fn handle_log_client(
    stream: TcpStream,
    nonce: String,
    cb: Option<EventCallback>,
    init_tx_slot: Arc<Mutex<Option<oneshot::Sender<InitHandshakeOutcome>>>>,
    _permit: OwnedSemaphorePermit,
) {
    let mut reader: BufReader<TcpStream> = BufReader::new(stream);
    let mut authenticated: bool = false;

    loop {
        let line_outcome: std::io::Result<BoundedLine> = if authenticated {
            read_bounded_line(&mut reader, MAX_NDJSON_LINE_BYTES, false).await
        } else {
            match tokio::time::timeout(
                Duration::from_secs(UNAUTH_FIRST_LINE_TIMEOUT_SECS),
                read_bounded_line(&mut reader, MAX_NDJSON_LINE_BYTES, true),
            )
            .await
            {
                Ok(res) => res,
                Err(_) => break,
            }
        };

        let raw_line: Vec<u8> = match line_outcome {
            Ok(BoundedLine::Line(bytes)) => bytes,
            Ok(BoundedLine::TooLong) => {
                if !authenticated {
                    break;
                }
                continue;
            }
            Ok(BoundedLine::Eof) | Err(_) => break,
        };

        let Ok(line_str): std::result::Result<&str, std::str::Utf8Error> =
            std::str::from_utf8(&raw_line)
        else {
            if !authenticated {
                break;
            }
            continue;
        };

        let trimmed: &str = line_str.trim();
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

        if !constant_time_eq(packet.n.as_bytes(), nonce.as_bytes()) {
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
                        file_name: packet.f.unwrap_or_else(|| "bundle".to_string()),
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
                let file_name: String = packet.f.unwrap_or_else(|| "bundle".to_string());
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
                    let _ = tx.send(InitHandshakeOutcome::Error { file_name, message });
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
    let session_nonce: String = generate_session_nonce()?;

    let (init_tx, init_rx): (
        oneshot::Sender<InitHandshakeOutcome>,
        oneshot::Receiver<InitHandshakeOutcome>,
    ) = oneshot::channel::<InitHandshakeOutcome>();
    let (shutdown_tx, mut shutdown_rx): (oneshot::Sender<()>, oneshot::Receiver<()>) =
        oneshot::channel::<()>();

    let shared_init_tx: Arc<Mutex<Option<oneshot::Sender<InitHandshakeOutcome>>>> =
        Arc::new(Mutex::new(Some(init_tx)));
    let expected_nonce: String = session_nonce.clone();
    let client_limit: Arc<Semaphore> = Arc::new(Semaphore::new(MAX_CONCURRENT_LOG_CLIENTS));

    let task: JoinHandle<()> = tokio::spawn(async move {
        let mut client_tasks: JoinSet<()> = JoinSet::new();
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    break;
                }
                _ = client_tasks.join_next(), if !client_tasks.is_empty() => {}
                accept_res = listener.accept() => {
                    let Ok((stream, _addr)): std::io::Result<(TcpStream, SocketAddr)> = accept_res else {
                        break;
                    };
                    let Ok(permit): std::result::Result<OwnedSemaphorePermit, tokio::sync::TryAcquireError> =
                        Arc::clone(&client_limit).try_acquire_owned()
                    else {
                        drop(stream);
                        continue;
                    };
                    let nonce: String = expected_nonce.clone();
                    let cb: Option<EventCallback> = on_event.clone();
                    let init_tx_slot: Arc<Mutex<Option<oneshot::Sender<InitHandshakeOutcome>>>> =
                        Arc::clone(&shared_init_tx);

                    let _: tokio::task::AbortHandle = client_tasks.spawn(handle_log_client(
                        stream,
                        nonce,
                        cb,
                        init_tx_slot,
                        permit,
                    ));
                }
            }
        }
        client_tasks.abort_all();
        while client_tasks.join_next().await.is_some() {}
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
