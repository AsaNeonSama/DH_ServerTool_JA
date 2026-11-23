use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::response::Response;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::structs::crypto::AesGcmSessionCipher;
use crate::structs::languages::ApiCode;

/// Callback sink for formatted Tab 1 (Web Console) log lines.
pub type WebLogSink = Arc<dyn Fn(String) + Send + Sync>;

pub struct CachedPage {
    pub raw: &'static [u8],
    pub gzip: &'static [u8],
    pub br: &'static [u8],
}

/// Encrypted JSON response body returned by Web Console POST API endpoints.
/// Carries only HTTP-like numeric `status` and a language-neutral `code` (`ApiCode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiResultPayload {
    pub status: u16,
    pub code: ApiCode,
}

pub enum DecryptedPostRequest {
    Ready {
        data: Map<String, Value>,
        client_ip: String,
    },
    EarlyResponse(Response),
}

pub struct WebConsoleAuth {
    pub(crate) auth_id: String,
    pub(crate) cipher: AesGcmSessionCipher,
    pub(crate) log_sink: WebLogSink,
    pub(crate) pending_challenges: Mutex<VecDeque<(String, Instant)>>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(crate) enum ConsoleBridgePayload<'a> {
    ExecuteConsoleCommand {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
        command: &'a str,
    },
    SetHumanProperty {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
        #[serde(rename = "propertyName")]
        property_name: &'a str,
        #[serde(rename = "propertyValue")]
        property_value: serde_json::Number,
    },
    AddInventoryItem {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
        #[serde(rename = "itemBlueprintPath")]
        item_blueprint_path: &'a str,
        #[serde(rename = "itemCount")]
        item_count: i64,
    },
    SetStorageLimit {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
        #[serde(rename = "storageLimit")]
        storage_limit: i64,
    },
    SendThrallMessage {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
        message: &'a str,
    },
    KickPlayer {
        #[serde(rename = "targetRole")]
        target_role: &'a str,
    },
}

#[derive(Debug, Clone)]
pub struct ConsoleBridge {
    pub(crate) port: u16,
    pub(crate) timeout: Duration,
}

pub struct WebConsoleOptions {
    pub web_port: u16,
    pub console_patch_port: u16,
    pub auto_broadcast: bool,
    pub public_ip: Option<String>,
    pub log_sink: WebLogSink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebConsoleDisplayInfo {
    pub port: u16,
    pub console_patch_port: u16,
    pub auto_broadcast: bool,
    pub public_url: String,
    pub local_url: String,
}

pub struct WebConsoleState {
    pub auth: WebConsoleAuth,
    pub bridge: ConsoleBridge,
    pub auto_broadcast: bool,
    pub log_sink: WebLogSink,
}

pub struct WebConsoleHandle {
    pub auth_id: String,
    pub auth_token: String,
    pub port: u16,
    pub display_info: WebConsoleDisplayInfo,
    pub(crate) shutdown_token: CancellationToken,
    pub(crate) log_sink: WebLogSink,
    pub(crate) stopped: Arc<AtomicBool>,
    pub(crate) task_handle: Option<JoinHandle<()>>,
}

pub struct ApiRouteContext<'a> {
    pub bridge: &'a ConsoleBridge,
    pub auto_broadcast: bool,
    pub log_sink: &'a WebLogSink,
}
