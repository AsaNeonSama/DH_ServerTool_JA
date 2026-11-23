use crate::structs::web_console::CachedPage;

pub const PLAYER_NAME_LIST: [&str; 8] = [
    "Captain",
    "Chaplain",
    "Cook",
    "Doctor",
    "Engineer",
    "Hunter",
    "Marine",
    "Navigator",
];

pub const COOKIE_NAME: &str = "dh_console_auth";
pub const COOKIE_MAX_AGE_SECONDS: u64 = 315_360_000;
pub const MAX_ENCRYPTED_BODY_BYTES: usize = 65_536;

pub const DEFAULT_TCP_TIMEOUT_MS: u64 = 1500;
pub const BONE_DAGGER_ITEM: &str =
    "/Game/Blueprints/Inventory/Totem/BP_BoneDagger_Inventory.BP_BoneDagger_Inventory_C";

pub const MAX_LOG_TEXT_CHARS: usize = 64;
pub const HIGH_STAT_WARNING_THRESHOLD: f64 = 500.0;
pub const MAX_SAFE_INT_F64: f64 = 9_007_199_254_740_991.0;

pub mod status_code {
    pub const SUCCESS: u16 = 200;
    pub const BAD_REQUEST: u16 = 400;
    pub const SERVER_ERROR: u16 = 504;
    pub const INVALID_VALUE: u16 = 416;
    pub const PARTIAL_SUCCESS: u16 = 207;
}

pub mod paths {
    pub const FAVICON: &str = "/favicon.ico";
    pub const ROOT: &str = "/";
    pub const UN_AUTH: &str = "/dh-console/un-auth";
    pub const HOME_PAGE: &str = "/dh-console/home_page";
    pub const CONSOLE_COMMAND: &str = "/dh-console/console_command";
    pub const SET_HUMAN: &str = "/dh-console/set_human";
    pub const ADD_ITEM: &str = "/dh-console/add_item";
    pub const SET_STORAGE: &str = "/dh-console/set_storage";
    pub const SEND_THRALL_MSG: &str = "/dh-console/send_thrall_msg";
    pub const BROADCAST: &str = "/dh-console/send_thrall_msg/broadcast";
    pub const KICK_PLAYER: &str = "/dh-console/kick_player";
}

pub const CONTROL_PAGE_CACHE: CachedPage = CachedPage {
    raw: include_bytes!(concat!(env!("OUT_DIR"), "/dh_console.raw.html")),
    gzip: include_bytes!(concat!(env!("OUT_DIR"), "/dh_console.html.gz")),
    br: include_bytes!(concat!(env!("OUT_DIR"), "/dh_console.html.br")),
};

pub const UN_AUTH_PAGE_CACHE: CachedPage = CachedPage {
    raw: include_bytes!(concat!(env!("OUT_DIR"), "/un_auth.raw.html")),
    gzip: include_bytes!(concat!(env!("OUT_DIR"), "/un_auth.html.gz")),
    br: include_bytes!(concat!(env!("OUT_DIR"), "/un_auth.html.br")),
};

pub mod auth_bootstrap_tokens {
    pub const CHALLENGE: &str = "/*__DH_AUTH_CHALLENGE__*/\"\"";
    pub const VERIFY_PATH: &str = "/*__DH_VERIFY_PATH__*/\"\"";
    pub const UN_AUTH_PATH: &str = "/*__DH_UN_AUTH_PATH__*/\"\"";
    pub const HOME_PAGE_PATH: &str = "/*__DH_HOME_PAGE_PATH__*/\"\"";
}

pub const AUTH_BOOTSTRAP_TEMPLATE: &str =
    include_str!(concat!(env!("OUT_DIR"), "/auth_bootstrap.template.html"));

pub const FAVICON_BYTES: &[u8] = include_bytes!("../../../../assets/icon.ico");

pub const DH_CIPHER_JS: &str = include_str!("../../../../assets/console/dh_cipher.js");
