use std::collections::HashMap;
use std::path::Path;

use dh_gadget_injector::{build_bundle_js, preprocess_mod_source, BundleBuildParams, PatchMod};

#[test]
fn preprocess_strips_bom_and_replaces_tokens() {
    let mut tokens = HashMap::new();
    tokens.insert("__CONSOLE_PATCH_PORT__".to_string(), "7893".to_string());
    let src = "\u{FEFF}var port = __CONSOLE_PATCH_PORT__;";
    assert_eq!(preprocess_mod_source(src, &tokens), "var port = 7893;");
}

#[test]
fn build_bundle_js_contains_all_blocks_and_mod_payload() {
    let mods = vec![PatchMod {
        id: "default:console".to_string(),
        file_name: "console.js".to_string(),
        label: "Console".to_string(),
        source: "\u{FEFF}var p = __CONSOLE_PATCH_PORT__; console.log('ok');".to_string(),
    }];
    let mut tokens = HashMap::new();
    tokens.insert("__CONSOLE_PATCH_PORT__".to_string(), "7893".to_string());

    let js = build_bundle_js(&BundleBuildParams {
        mods: &mods,
        tokens: &tokens,
        log_port: 12345,
        session_nonce: "0123456789abcdef0123456789abcdef",
        patch_log_path: Path::new(r"C:\Temp\patch_runtime.log"),
    })
    .expect("build_bundle_js");

    assert!(js.contains("exceptions:\"propagate\""));
    assert!(js.contains("__compatModule = Object.create(Module)"));
    assert!(js.contains("__compatMemory = Object.create(Memory)"));
    assert!(js.contains("var __LOG_PORT = 12345;"));
    assert!(js.contains("0123456789abcdef0123456789abcdef"));
    assert!(js.contains("var p = 7893;"));
    assert!(!js.contains("__CONSOLE_PATCH_PORT__"));
    assert!(!js.contains('\u{FEFF}'));
}
