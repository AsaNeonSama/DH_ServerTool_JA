pub const TARGET_EXE_FILE_NAME: &str = "DreadHunger-Win64-Shipping.exe";

pub const RELATIVE_EXE_CANDIDATES: &[&str] = &[
    "./DreadHunger/Binaries/Win64/DreadHunger-Win64-Shipping.exe",
    "./DreadHunger-Win64-Shipping.exe",
    "../DreadHunger/Binaries/Win64/DreadHunger-Win64-Shipping.exe",
    "../DreadHunger-Win64-Shipping.exe",
];

pub const STEAM_COMMON_SUFFIX: &str =
    r"steamapps\common\Dread Hunger\DreadHunger\Binaries\Win64\DreadHunger-Win64-Shipping.exe";

pub const STEAM_LIBRARY_VDF_RELATIVE_PATHS: &[&str] = &[
    r"Program Files (x86)\Steam\steamapps\libraryfolders.vdf",
    r"Program Files\Steam\steamapps\libraryfolders.vdf",
    r"Steam\steamapps\libraryfolders.vdf",
];

pub const STEAM_ROOT_CANDIDATES: &[&str] = &[
    r"Program Files (x86)\Steam",
    r"Program Files\Steam",
    r"SteamLibrary",
    r"Steam",
    r"Games\SteamLibrary",
];

pub const EOS_PLAN_A_DESCRIPTION: &str = "EOS再接続・同ポート再起動修正 (通常版 / 19パッチ)";
pub const EOS_PLAN_B_DESCRIPTION: &str =
    "EOS再接続修正 + ロビーURI非公開化 (20パッチ / Plan A と排他)";
pub const VOICE_CLIENT_DESCRIPTION: &str =
    "自有音声クライアント (追加ポート不要・同一UDPポート3D音声 / ロビーEOS自動切替)";
