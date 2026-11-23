use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::BuildHasher;

use crate::consts::prelude::BUNDLE_RUNTIME_TEMPLATE;
pub use crate::consts::prelude::WINDOWS_NATIVE_FUNCTION_PRELUDE;
use crate::error::Result;
pub use crate::structs::prelude::BundleBuildParams;
use crate::structs::prelude::SerializedMod;
use crate::PatchMod;

/// Strips a leading UTF-8 BOM (`\u{FEFF}`) and applies explicit placeholder replacements.
#[must_use]
pub fn preprocess_mod_source<S: BuildHasher>(
    source: &str,
    tokens: &HashMap<String, String, S>,
) -> String {
    let mut cleaned: String = source
        .strip_prefix('\u{FEFF}')
        .unwrap_or(source)
        .to_string();
    for (token, value) in tokens {
        if cleaned.contains(token.as_str()) {
            cleaned = cleaned.replace(token.as_str(), value.as_str());
        }
    }
    cleaned
}

/// Assembles the aggregated `bundle.js` executed by `frida-gadget-17.27.4` (QuickJS):
/// - Block A: `WINDOWS_NATIVE_FUNCTION_PRELUDE` (`exceptions: "propagate"`)
/// - Block B: Frida 16 -> 17 `Object.create(Module)` / `Object.create(Memory)` compatibility shim
/// - Block C: Loopback NDJSON log/handshake client with early queue & `new File(..., 'a')` fallback
/// - Block D: Isolated per-MOD `new Function("console", "send", "Module", "Memory", mod.source)` runner
pub fn build_bundle_js(params: &BundleBuildParams<'_>) -> Result<String> {
    let serialized_mods: Vec<SerializedMod<'_>> = params
        .mods
        .iter()
        .map(|m: &PatchMod| SerializedMod {
            id: &m.id,
            file_name: &m.file_name,
            source: preprocess_mod_source(&m.source, params.tokens),
        })
        .collect();

    let mods_json: String = serde_json::to_string(&serialized_mods)?;
    let nonce_json: String = serde_json::to_string(params.session_nonce)?;
    let log_path_json: String = serde_json::to_string(&params.patch_log_path.to_string_lossy())?;

    let mut out: String = String::with_capacity(mods_json.len() + 8192);
    out.push_str(WINDOWS_NATIVE_FUNCTION_PRELUDE);
    out.push_str("(function() {\n");
    let _ = writeln!(out, "  var __LOG_PORT = {};", params.log_port);
    let _ = writeln!(out, "  var __SESSION_NONCE = {nonce_json};");
    let _ = writeln!(out, "  var __PATCH_LOG_PATH = {log_path_json};");
    let _ = writeln!(out, "  var __MODS = {mods_json};");

    out.push_str(BUNDLE_RUNTIME_TEMPLATE);
    out.push_str("})();\n");
    Ok(out)
}
