use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::structs::languages::ApiCode;

/// Structured log event payload delivered from local Admin Console dispatch to the GUI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleLogRecord {
    pub time: String,
    pub category: String,
    pub tone: String,
    pub detail: String,
}

/// Callback sink for structured in-app Admin Console log records.
pub type ConsoleLogSink = Arc<dyn Fn(ConsoleLogRecord) + Send + Sync>;

/// Result returned by local Admin Console command dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleActionPayload {
    pub status: u16,
    pub code: ApiCode,
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

pub struct ConsoleDispatchContext<'a> {
    pub bridge: &'a ConsoleBridge,
    pub auto_broadcast: bool,
    pub log_sink: &'a ConsoleLogSink,
}
