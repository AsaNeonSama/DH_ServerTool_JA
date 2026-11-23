use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::structs::session::PatchMod;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SerializedMod<'a> {
    pub(crate) id: &'a str,
    pub(crate) file_name: &'a str,
    pub(crate) source: String,
}

/// Parameters required to assemble a single-round `bundle.js` script.
pub struct BundleBuildParams<'a> {
    pub mods: &'a [PatchMod],
    pub tokens: &'a HashMap<String, String>,
    pub log_port: u16,
    pub session_nonce: &'a str,
    pub patch_log_path: &'a Path,
}
