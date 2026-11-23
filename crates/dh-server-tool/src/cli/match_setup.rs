use tokio_util::sync::CancellationToken;

use crate::cli::mod_selector::prompt_mod_selection;
use crate::colors;
use crate::messages::ja::cli as cli_msg;
use crate::terminal::{PromptError, Terminal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapOption {
    pub key: &'static str,
    pub id: &'static str,
    pub label: &'static str,
}

pub const MAP_OPTIONS: [MapOption; 3] = [
    MapOption {
        key: "1",
        id: "Departure_Persistent",
        label: cli_msg::MAP_LABEL_1,
    },
    MapOption {
        key: "2",
        id: "Expanse_Persistent",
        label: cli_msg::MAP_LABEL_2,
    },
    MapOption {
        key: "3",
        id: "Approach_Persistent",
        label: cli_msg::MAP_LABEL_3,
    },
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ServerSettings {
    pub max_players: u32,
    pub days_before_blizzard: u32,
    pub day_minutes: u32,
    pub predator_damage: f64,
    pub cold_intensity: f64,
    pub hunger_rate: f64,
    pub coal_burn_rate: f64,
    pub thralls: u32,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            max_players: 8,
            days_before_blizzard: 3,
            day_minutes: 9,
            predator_damage: 1.0,
            cold_intensity: 1.0,
            hunger_rate: 1.0,
            coal_burn_rate: 1.0,
            thralls: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchConfig {
    pub map_id: &'static str,
    pub map_label: &'static str,
    pub settings: ServerSettings,
    pub selected_option_numbers: Vec<u32>,
    pub selected_option_labels: Vec<String>,
    pub ue_map_argument: String,
}

#[must_use]
pub fn parse_map_choice(raw: &str) -> Option<MapOption> {
    let trimmed: &str = raw.trim();
    MAP_OPTIONS
        .iter()
        .copied()
        .find(|opt: &MapOption| opt.key == trimmed)
}

/// Parses an integer prompt entry within `[min, max]`, returning `default_value` for empty input.
#[must_use]
pub fn parse_u32_in_range(raw: &str, min: u32, max: u32, default_value: u32) -> Option<u32> {
    let trimmed: &str = raw.trim();
    if trimmed.is_empty() {
        return Some(default_value);
    }
    let parsed: u32 = trimmed.parse::<u32>().ok()?;
    if parsed < min || parsed > max {
        return None;
    }
    Some(parsed)
}

/// Parses a numeric prompt entry within `[min, max]`, returning `default_value` for empty input.
#[must_use]
pub fn parse_number_in_range(
    raw: &str,
    min: f64,
    max: f64,
    default_value: f64,
    integer_only: bool,
) -> Option<f64> {
    let trimmed: &str = raw.trim();
    if trimmed.is_empty() {
        return Some(default_value);
    }
    let parsed: f64 = trimmed.parse::<f64>().ok()?;
    if !parsed.is_finite() {
        return None;
    }
    if integer_only && parsed.fract() != 0.0 {
        return None;
    }
    if parsed < min || parsed > max {
        return None;
    }
    Some(parsed)
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

struct IntegerPromptSpec<'a> {
    label: &'a str,
    min: u32,
    max: u32,
    default_value: u32,
}

struct FloatPromptSpec<'a> {
    label: &'a str,
    min: f64,
    max: f64,
    default_value: f64,
}

async fn ask_u32_in_range(
    terminal: &Terminal,
    spec: &IntegerPromptSpec<'_>,
    cancel: Option<&CancellationToken>,
) -> Result<u32, PromptError> {
    let prompt: String = format!(
        "{} * {}({}-{}) {}[Enter={}]{}: ",
        colors::LIGHT_CYAN,
        spec.label,
        spec.min,
        spec.max,
        colors::LIGHT_BLACK,
        spec.default_value,
        colors::LIGHT_CYAN,
    );
    loop {
        let raw: String = terminal.ask(&prompt, cancel).await?;
        if let Some(val) = parse_u32_in_range(&raw, spec.min, spec.max, spec.default_value) {
            return Ok(val);
        }
        terminal.println(&format!(
            "{} * {}",
            colors::RED,
            cli_msg::INVALID_CHOICE_RETRY
        ));
    }
}

async fn ask_f64_in_range(
    terminal: &Terminal,
    spec: &FloatPromptSpec<'_>,
    cancel: Option<&CancellationToken>,
) -> Result<f64, PromptError> {
    let prompt: String = format!(
        "{} * {}({}-{}) {}[Enter={}]{}: ",
        colors::LIGHT_CYAN,
        spec.label,
        spec.min,
        spec.max,
        colors::LIGHT_BLACK,
        spec.default_value,
        colors::LIGHT_CYAN,
    );
    loop {
        let raw: String = terminal.ask(&prompt, cancel).await?;
        if let Some(val) =
            parse_number_in_range(&raw, spec.min, spec.max, spec.default_value, false)
        {
            return Ok(val);
        }
        terminal.println(&format!(
            "{} * {}",
            colors::RED,
            cli_msg::INVALID_CHOICE_RETRY
        ));
    }
}

pub async fn prompt_match_config(
    terminal: &Terminal,
    base_settings: &ServerSettings,
    cancel: Option<&CancellationToken>,
) -> Result<MatchConfig, PromptError> {
    // 1. Map selection
    let map_prompt: String = format!(
        "{} * {}({}{}{}): ",
        colors::LIGHT_YELLOW,
        cli_msg::MAP_PROMPT_PREFIX,
        colors::LIGHT_GREEN,
        cli_msg::MAP_PROMPT_CHOICES,
        colors::LIGHT_YELLOW,
    );
    let map_choice: MapOption = loop {
        let raw_map: String = terminal.ask(&map_prompt, cancel).await?;
        if let Some(choice) = parse_map_choice(&raw_map) {
            break choice;
        }
        terminal.println(&format!(
            "{} * {}",
            colors::RED,
            cli_msg::INVALID_CHOICE_RETRY
        ));
    };

    // 2. Server configuration (default or edit 8 parameters)
    terminal.println_empty();
    let mode_prompt: String = format!(
        "{} * {}({}{}{}): ",
        colors::LIGHT_YELLOW,
        cli_msg::SERVER_CONFIG_PROMPT_PREFIX,
        colors::LIGHT_GREEN,
        cli_msg::SERVER_CONFIG_CHOICES,
        colors::LIGHT_YELLOW,
    );
    let settings: ServerSettings = loop {
        let raw_mode: String = terminal.ask(&mode_prompt, cancel).await?;
        if raw_mode == "1" {
            break ServerSettings::default();
        }
        if raw_mode == "2" {
            terminal.println_empty();
            let max_players: u32 = ask_u32_in_range(
                terminal,
                &IntegerPromptSpec {
                    label: cli_msg::setting_labels::MAX_PLAYERS,
                    min: 1,
                    max: 8,
                    default_value: base_settings.max_players,
                },
                cancel,
            )
            .await?;
            let days_before_blizzard: u32 = ask_u32_in_range(
                terminal,
                &IntegerPromptSpec {
                    label: cli_msg::setting_labels::DAYS_BEFORE_BLIZZARD,
                    min: 2,
                    max: 7,
                    default_value: base_settings.days_before_blizzard,
                },
                cancel,
            )
            .await?;
            let day_minutes: u32 = ask_u32_in_range(
                terminal,
                &IntegerPromptSpec {
                    label: cli_msg::setting_labels::DAY_MINUTES,
                    min: 5,
                    max: 16,
                    default_value: base_settings.day_minutes,
                },
                cancel,
            )
            .await?;
            let predator_damage: f64 = ask_f64_in_range(
                terminal,
                &FloatPromptSpec {
                    label: cli_msg::setting_labels::PREDATOR_DAMAGE,
                    min: 0.25,
                    max: 3.0,
                    default_value: base_settings.predator_damage,
                },
                cancel,
            )
            .await?;
            let cold_intensity: f64 = ask_f64_in_range(
                terminal,
                &FloatPromptSpec {
                    label: cli_msg::setting_labels::COLD_INTENSITY,
                    min: 0.25,
                    max: 3.0,
                    default_value: base_settings.cold_intensity,
                },
                cancel,
            )
            .await?;
            let hunger_rate: f64 = ask_f64_in_range(
                terminal,
                &FloatPromptSpec {
                    label: cli_msg::setting_labels::HUNGER_RATE,
                    min: 0.25,
                    max: 3.0,
                    default_value: base_settings.hunger_rate,
                },
                cancel,
            )
            .await?;
            let coal_burn_rate: f64 = ask_f64_in_range(
                terminal,
                &FloatPromptSpec {
                    label: cli_msg::setting_labels::COAL_BURN_RATE,
                    min: 0.1,
                    max: 2.0,
                    default_value: base_settings.coal_burn_rate,
                },
                cancel,
            )
            .await?;
            let thralls: u32 = ask_u32_in_range(
                terminal,
                &IntegerPromptSpec {
                    label: cli_msg::setting_labels::THRALLS,
                    min: 0,
                    max: 8,
                    default_value: base_settings.thralls,
                },
                cancel,
            )
            .await?;

            break ServerSettings {
                max_players,
                days_before_blizzard,
                day_minutes,
                predator_damage,
                cold_intensity,
                hunger_rate,
                coal_burn_rate,
                thralls,
            };
        }
        terminal.println(&format!(
            "{} * {}",
            colors::RED,
            cli_msg::INVALID_CHOICE_RETRY
        ));
    };

    // 3. MOD selection loop
    terminal.println_empty();
    let (selected_option_numbers, selected_option_labels): (Vec<u32>, Vec<String>) =
        prompt_mod_selection(terminal, cancel).await?;
    let ue_map_argument: String = build_ue_map_argument(map_choice.id, &settings);

    Ok(MatchConfig {
        map_id: map_choice.id,
        map_label: map_choice.label,
        settings,
        selected_option_numbers,
        selected_option_labels,
        ue_map_argument,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_three_maps_and_rejects_invalid() {
        let m1 = parse_map_choice("1").unwrap();
        assert_eq!(m1.id, "Departure_Persistent");
        assert_eq!(m1.label, "山頂");

        let m2 = parse_map_choice("2").unwrap();
        assert_eq!(m2.id, "Expanse_Persistent");
        assert_eq!(m2.label, "広漠");

        let m3 = parse_map_choice("3").unwrap();
        assert_eq!(m3.id, "Approach_Persistent");
        assert_eq!(m3.label, "進入路");

        assert!(parse_map_choice("0").is_none());
        assert!(parse_map_choice("4").is_none());
        assert!(parse_map_choice("").is_none());
    }

    #[test]
    fn builds_ue_map_argument_matching_ts_format() {
        let defaults = ServerSettings::default();
        assert_eq!(
            build_ue_map_argument("Departure_Persistent", &defaults),
            "Departure_Persistent?maxplayers=8?daysbeforeblizzard=3?dayminutes=9?predatordamage=1?coldintensity=1?hungerrate=1?coalburnrate=1?thralls=2"
        );

        let custom = ServerSettings {
            max_players: 6,
            days_before_blizzard: 4,
            day_minutes: 10,
            predator_damage: 0.75,
            cold_intensity: 1.5,
            hunger_rate: 1.25,
            coal_burn_rate: 0.5,
            thralls: 1,
        };
        assert_eq!(
            build_ue_map_argument("Expanse_Persistent", &custom),
            "Expanse_Persistent?maxplayers=6?daysbeforeblizzard=4?dayminutes=10?predatordamage=0.75?coldintensity=1.5?hungerrate=1.25?coalburnrate=0.5?thralls=1"
        );
    }

    #[test]
    fn validates_integer_and_float_ranges() {
        assert_eq!(parse_u32_in_range("", 1, 8, 8), Some(8));
        assert_eq!(parse_u32_in_range("1", 1, 8, 8), Some(1));
        assert_eq!(parse_u32_in_range("8", 1, 8, 8), Some(8));
        assert_eq!(parse_u32_in_range("0", 1, 8, 8), None);
        assert_eq!(parse_u32_in_range("9", 1, 8, 8), None);
        assert_eq!(parse_u32_in_range("3.5", 1, 8, 8), None);

        assert_eq!(parse_number_in_range("", 1.0, 8.0, 8.0, true), Some(8.0));
        assert_eq!(parse_number_in_range("1", 1.0, 8.0, 8.0, true), Some(1.0));
        assert_eq!(parse_number_in_range("8", 1.0, 8.0, 8.0, true), Some(8.0));
        assert_eq!(parse_number_in_range("0", 1.0, 8.0, 8.0, true), None);
        assert_eq!(parse_number_in_range("9", 1.0, 8.0, 8.0, true), None);
        assert_eq!(parse_number_in_range("3.5", 1.0, 8.0, 8.0, true), None);
        assert_eq!(
            parse_number_in_range("0.25", 0.25, 3.0, 1.0, false),
            Some(0.25)
        );
        assert_eq!(
            parse_number_in_range("0.1", 0.1, 2.0, 1.0, false),
            Some(0.1)
        );
        assert_eq!(parse_number_in_range("0.09", 0.1, 2.0, 1.0, false), None);
        assert_eq!(parse_number_in_range("NaN", 0.1, 2.0, 1.0, false), None);
        assert_eq!(parse_number_in_range("inf", 0.1, 2.0, 1.0, false), None);
        assert_eq!(parse_number_in_range("abc", 0.1, 2.0, 1.0, false), None);
    }
}
