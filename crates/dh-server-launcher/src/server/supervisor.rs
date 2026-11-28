use std::path::{Component, Path, PathBuf};

pub use crate::consts::server::{
    DEFAULT_COAL_BURN_RATE, DEFAULT_COLD_INTENSITY, DEFAULT_DAYS_BEFORE_BLIZZARD,
    DEFAULT_DAY_MINUTES, DEFAULT_HUNGER_RATE, DEFAULT_MAX_PLAYERS, DEFAULT_PREDATOR_DAMAGE,
    DEFAULT_THRALLS, DETACH_EXIT_GRACE_MS, INJECTION_INIT_TIMEOUT_SECS, LEGACY_GADGET_DLL_FILENAME,
    MAP_OPTIONS, STARTUP_RETRY_DELAY_MS, UDP_MISSING_STREAK_LIMIT, UE_PROJECT_NAME,
};
pub use crate::structs::server::{LaunchPlan, MapOption, MatchConfig, ServerSettings};

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            max_players: DEFAULT_MAX_PLAYERS,
            days_before_blizzard: DEFAULT_DAYS_BEFORE_BLIZZARD,
            day_minutes: DEFAULT_DAY_MINUTES,
            predator_damage: DEFAULT_PREDATOR_DAMAGE,
            cold_intensity: DEFAULT_COLD_INTENSITY,
            hunger_rate: DEFAULT_HUNGER_RATE,
            coal_burn_rate: DEFAULT_COAL_BURN_RATE,
            thralls: DEFAULT_THRALLS,
        }
    }
}

impl LaunchPlan {
    #[must_use]
    pub fn from_parts(
        console_patch_port: u16,
        server_ports: [u16; 2],
        switch_port_on_restart: bool,
        match_config: MatchConfig,
    ) -> Self {
        Self {
            console_patch_port,
            server_ports,
            switch_port_on_restart,
            map_id: match_config.map_id,
            map_label_code: match_config.map_label_code,
            settings: match_config.settings,
            selected_content_option_numbers: match_config.selected_content_option_numbers,
            selected_content_option_labels: match_config.selected_content_option_labels,
            selected_option_numbers: match_config.selected_option_numbers,
            selected_option_labels: match_config.selected_option_labels,
            ue_map_argument: match_config.ue_map_argument,
        }
    }
}

#[must_use]
pub fn build_ue_map_argument(map_id: &str, settings: &ServerSettings) -> String {
    format!(
        "{map_id}?maxplayers={}?daysbeforeblizzard={}?dayminutes={}?predatordamage={}?coldintensity={}?hungerrate={}?coalburnrate={}?thralls={}",
        settings.max_players,
        settings.days_before_blizzard,
        settings.day_minutes,
        settings.predator_damage,
        settings.cold_intensity,
        settings.hunger_rate,
        settings.coal_burn_rate,
        settings.thralls,
    )
}

#[must_use]
pub fn alternate_port(ports: [u16; 2], current: u16) -> u16 {
    if current == ports[0] {
        ports[1]
    } else {
        ports[0]
    }
}

#[must_use]
pub fn next_server_port(ports: [u16; 2], current: u16, switch_port_on_restart: bool) -> u16 {
    if switch_port_on_restart {
        alternate_port(ports, current)
    } else {
        current
    }
}

/// Resolves a relative executable path against `base_dir` without producing a Windows
/// `\\?\` verbatim prefix (which UE4's launch-directory parser can mishandle).
#[must_use]
pub fn resolve_clean_path(path: &Path, base_dir: &Path) -> PathBuf {
    let joined: PathBuf = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    };
    let mut normalized: PathBuf = PathBuf::new();
    for comp in joined.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                let _: bool = normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    let normalized_str: std::borrow::Cow<'_, str> = normalized.to_string_lossy();
    if let Some(stripped) = normalized_str.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        normalized
    }
}
