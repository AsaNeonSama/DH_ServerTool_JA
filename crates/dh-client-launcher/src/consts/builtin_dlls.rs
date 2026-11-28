include!(concat!(env!("OUT_DIR"), "/builtin_dlls_meta.rs"));

pub const EMBEDDED_EOS_FIX_PLAN_A_ZST: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/eos_fix_plan_a.dll.zst"));
pub const EMBEDDED_EOS_FIX_PLAN_B_ZST: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/eos_fix_plan_b.dll.zst"));
pub const EMBEDDED_VOICE_CLIENT_ZST: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/dh_voice_client.dll.zst"));

pub const BUILTIN_CACHE_APP_DIR: &str = "DH_ClientLauncher_JA";
pub const BUILTIN_CACHE_SUBDIR: &str = "builtin_dlls";
pub const BUILTIN_CACHE_FALLBACK_DIR: &str = ".dh_client_cache";

pub const BUILTIN_EOS_FIX_FILE_NAME: &str = "eos_fix.dll";
pub const BUILTIN_VOICE_CLIENT_FILE_NAME: &str = "dh_voice_client.dll";

pub const BUILTIN_EOS_PLAN_A_SUBDIR_PREFIX: &str = "eos_fix_plan_a_";
pub const BUILTIN_EOS_PLAN_B_SUBDIR_PREFIX: &str = "eos_fix_plan_b_";
pub const BUILTIN_VOICE_SUBDIR_PREFIX: &str = "voice_chat_";

pub const BUILTIN_EOS_PLAN_A_ID: &str = "eos_fix_plan_a";
pub const BUILTIN_EOS_PLAN_B_ID: &str = "eos_fix_plan_b";
pub const BUILTIN_VOICE_ID: &str = "voice_chat";

pub const BUILTIN_EOS_PLAN_A_LABEL: &str = "EOS修復 Plan A (標準・19パッチ)";
pub const BUILTIN_EOS_PLAN_B_LABEL: &str = "EOS修復 Plan B (非公開URI・20パッチ)";
pub const BUILTIN_VOICE_LABEL: &str = "UEネイティブボイス (voice_chat)";
