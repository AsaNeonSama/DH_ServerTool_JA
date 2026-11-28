/// Embedded zstd-22 compressed `frida-gadget-17.27.4-windows-x86_64.dll`.
pub const EMBEDDED_GADGET_ZST: &[u8] = include_bytes!(env!("DH_FRIDA_GADGET_ZST_PATH"));
/// Frida Gadget version string embedded at build time.
pub const FRIDA_GADGET_VERSION: &str = env!("DH_FRIDA_GADGET_VERSION");
/// Expected lowercase hex SHA-256 of the uncompressed Frida Gadget DLL.
pub const FRIDA_GADGET_SHA256: &str = env!("DH_FRIDA_GADGET_SHA256");

pub const CACHE_APP_DIR_NAME: &str = "DH_Tool_JA";
pub const FALLBACK_LOCAL_CACHE_DIR: &str = ".dh_cache";
pub const RUN_DIR_PREFIX: &str = "run_";
pub const GADGET_DLL_FILE_NAME: &str = "gadget.dll";
pub const GADGET_CONFIG_FILE_NAME: &str = "gadget.config";
pub const BUNDLE_JS_FILE_NAME: &str = "bundle.js";
pub const CLEANUP_RETRY_ATTEMPTS: u8 = 5;
pub const CLEANUP_RETRY_DELAY_MS: u64 = 60;
