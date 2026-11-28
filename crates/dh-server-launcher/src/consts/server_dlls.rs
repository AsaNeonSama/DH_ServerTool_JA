include!(concat!(env!("OUT_DIR"), "/builtin_server_dlls.rs"));

pub const EMBEDDED_VOICE_SERVER_DLL_ZST: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/dh_voice_server.dll.zst"));

pub const VOICE_SERVER_DLL_FILE_NAME: &str = "dh_voice_server.dll";
pub const VOICE_SERVER_DLL_SUBDIR_PREFIX: &str = "voice_server_";
pub const SERVER_DLL_CACHE_APP_DIR: &str = "DH_Tool_JA";
pub const SERVER_DLL_CACHE_SUBDIR: &str = "server_dlls";
pub const SERVER_DLL_CACHE_FALLBACK_DIR: &str = ".dh_cache";
pub const SHA256_READ_BUFFER_SIZE: usize = 8192;
