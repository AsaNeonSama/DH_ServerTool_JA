use tokio_util::sync::CancellationToken;

use crate::cli::mod_selector::{prompt_content_mod_selection, prompt_mod_selection};
use crate::colors;
pub use crate::consts::cli::MAP_OPTIONS;
use crate::consts::cli::{
    DEFAULT_COAL_BURN_RATE, DEFAULT_COLD_INTENSITY, DEFAULT_DAYS_BEFORE_BLIZZARD,
    DEFAULT_DAY_MINUTES, DEFAULT_HUNGER_RATE, DEFAULT_MAX_PLAYERS, DEFAULT_PREDATOR_DAMAGE,
    DEFAULT_THRALLS,
};
use crate::languages;
use crate::structs::cli::{FloatPromptSpec, IntegerPromptSpec};
pub use crate::structs::cli::{MapOption, MatchConfig, ServerSettings};
use crate::structs::languages::CliMsgCode;
use crate::terminal::{PromptError, Terminal};

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

async fn ask_u32_in_range(
    terminal: &Terminal,
    spec: &IntegerPromptSpec,
    cancel: Option<&CancellationToken>,
) -> Result<u32, PromptError> {
    let prompt: String = format!(
        "{} * {}({}-{}) {}[Enter={}]{}: ",
        colors::LIGHT_CYAN,
        languages::cli_text(spec.label_code),
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
            languages::cli_text(CliMsgCode::InvalidChoiceRetry)
        ));
    }
}

async fn ask_f64_in_range(
    terminal: &Terminal,
    spec: &FloatPromptSpec,
    cancel: Option<&CancellationToken>,
) -> Result<f64, PromptError> {
    let prompt: String = format!(
        "{} * {}({}-{}) {}[Enter={}]{}: ",
        colors::LIGHT_CYAN,
        languages::cli_text(spec.label_code),
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
            languages::cli_text(CliMsgCode::InvalidChoiceRetry)
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
        languages::cli_text(CliMsgCode::MapPromptPrefix),
        colors::LIGHT_GREEN,
        languages::cli_text(CliMsgCode::MapPromptChoices),
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
            languages::cli_text(CliMsgCode::InvalidChoiceRetry)
        ));
    };

    // 2. Server configuration (default or edit 8 parameters)
    terminal.println_empty();
    let mode_prompt: String = format!(
        "{} * {}({}{}{}): ",
        colors::LIGHT_YELLOW,
        languages::cli_text(CliMsgCode::ServerConfigPromptPrefix),
        colors::LIGHT_GREEN,
        languages::cli_text(CliMsgCode::ServerConfigChoices),
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
                    label_code: CliMsgCode::SettingMaxPlayers,
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
                    label_code: CliMsgCode::SettingDaysBeforeBlizzard,
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
                    label_code: CliMsgCode::SettingDayMinutes,
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
                    label_code: CliMsgCode::SettingPredatorDamage,
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
                    label_code: CliMsgCode::SettingColdIntensity,
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
                    label_code: CliMsgCode::SettingHungerRate,
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
                    label_code: CliMsgCode::SettingCoalBurnRate,
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
                    label_code: CliMsgCode::SettingThralls,
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
            languages::cli_text(CliMsgCode::InvalidChoiceRetry)
        ));
    };

    // 3. Content MOD selection (client .pak required) — FIRST
    terminal.println_empty();
    let (selected_content_option_numbers, selected_content_option_labels): (Vec<u32>, Vec<String>) =
        prompt_content_mod_selection(terminal, cancel).await?;

    // 4. Server-only JS MOD selection (no client .pak required) — SECOND
    terminal.println_empty();
    let (selected_option_numbers, selected_option_labels): (Vec<u32>, Vec<String>) =
        prompt_mod_selection(terminal, cancel).await?;
    let ue_map_argument: String = build_ue_map_argument(map_choice.id, &settings);

    Ok(MatchConfig {
        map_id: map_choice.id,
        map_label_code: map_choice.label_code,
        settings,
        selected_content_option_numbers,
        selected_content_option_labels,
        selected_option_numbers,
        selected_option_labels,
        ue_map_argument,
    })
}
