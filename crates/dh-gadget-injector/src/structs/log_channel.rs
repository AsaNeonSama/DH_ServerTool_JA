use serde::Deserialize;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

#[derive(Debug, Deserialize)]
pub(crate) struct NdjsonPacket {
    pub(crate) n: String,
    pub(crate) t: String,
    #[serde(default)]
    pub(crate) l: Option<String>,
    #[serde(default)]
    pub(crate) f: Option<String>,
    #[serde(default)]
    pub(crate) m: Option<String>,
}

/// Initialization outcome reported by the Gadget bundle over the loopback channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitHandshakeOutcome {
    Ok,
    Error { file_name: String, message: String },
}

pub(crate) enum BoundedLine {
    Eof,
    Line(Vec<u8>),
    TooLong,
}

/// Active loopback NDJSON log & handshake TCP server (`127.0.0.1:0`).
#[derive(Debug)]
pub struct LogServerHandle {
    pub(crate) port: u16,
    pub(crate) session_nonce: String,
    pub(crate) shutdown_tx: Option<oneshot::Sender<()>>,
    pub(crate) task: Option<JoinHandle<()>>,
}
