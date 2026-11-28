use serde::{Deserialize, Serialize};

/// Response returned by executable auto-detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDetectResponse {
    pub found: Option<String>,
    pub checked_count: usize,
}
