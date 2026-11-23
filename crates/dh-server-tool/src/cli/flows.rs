use tokio_util::sync::CancellationToken;

use crate::cli::match_setup::{prompt_match_config, MatchConfig, ServerSettings};
use crate::colors;
use crate::config::{is_valid_port_number, AppConfig, MAX_VALID_PORT, MIN_VALID_PORT};
use crate::messages::ja::cli as cli_msg;
use crate::terminal::{PromptError, Terminal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebConsoleSetupConfig {
    pub web_console_port: u16,
    pub console_patch_port: u16,
    pub auto_broadcast: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LaunchPlan {
    pub web_console_port: u16,
    pub console_patch_port: u16,
    pub auto_broadcast: bool,
    pub server_ports: [u16; 2],
    pub map_id: &'static str,
    pub map_label: &'static str,
    pub settings: ServerSettings,
    pub selected_option_numbers: Vec<u32>,
    pub selected_option_labels: Vec<String>,
    pub ue_map_argument: String,
}

impl LaunchPlan {
    #[must_use]
    pub fn from_parts(
        web_config: WebConsoleSetupConfig,
        server_ports: [u16; 2],
        match_config: MatchConfig,
    ) -> Self {
        Self {
            web_console_port: web_config.web_console_port,
            console_patch_port: web_config.console_patch_port,
            auto_broadcast: web_config.auto_broadcast,
            server_ports,
            map_id: match_config.map_id,
            map_label: match_config.map_label,
            settings: match_config.settings,
            selected_option_numbers: match_config.selected_option_numbers,
            selected_option_labels: match_config.selected_option_labels,
            ue_map_argument: match_config.ue_map_argument,
        }
    }

    #[must_use]
    pub fn with_match_config(&self, match_config: MatchConfig) -> Self {
        Self {
            web_console_port: self.web_console_port,
            console_patch_port: self.console_patch_port,
            auto_broadcast: self.auto_broadcast,
            server_ports: self.server_ports,
            map_id: match_config.map_id,
            map_label: match_config.map_label,
            settings: match_config.settings,
            selected_option_numbers: match_config.selected_option_numbers,
            selected_option_labels: match_config.selected_option_labels,
            ue_map_argument: match_config.ue_map_argument,
        }
    }
}

#[must_use]
pub fn parse_port_input(raw: &str, fallback: u16) -> Option<u16> {
    let trimmed: &str = raw.trim();
    let value: u16 = if trimmed.is_empty() {
        fallback
    } else {
        trimmed.parse::<u16>().ok()?
    };
    is_valid_port_number(value).then_some(value)
}

pub fn validate_web_port(raw: &str, fallback: u16, console_patch_port: u16) -> Result<u16, String> {
    let Some(parsed): Option<u16> = parse_port_input(raw, fallback) else {
        return Err(cli_msg::port_range_error(MIN_VALID_PORT, MAX_VALID_PORT));
    };
    if parsed == console_patch_port {
        return Err(cli_msg::web_port_conflict_error(console_patch_port));
    }
    Ok(parsed)
}

pub fn validate_server_ports(
    raw1: &str,
    raw2: &str,
    default_ports: [u16; 2],
    web_config: WebConsoleSetupConfig,
) -> Result<[u16; 2], String> {
    let (Some(p1), Some(p2)): (Option<u16>, Option<u16>) = (
        parse_port_input(raw1, default_ports[0]),
        parse_port_input(raw2, default_ports[1]),
    ) else {
        return Err(cli_msg::port_range_error(MIN_VALID_PORT, MAX_VALID_PORT));
    };
    if p1 == p2 {
        return Err(cli_msg::SERVER_PORT_DUPLICATE_ERROR.to_string());
    }
    if p1 == web_config.web_console_port
        || p2 == web_config.web_console_port
        || p1 == web_config.console_patch_port
        || p2 == web_config.console_patch_port
    {
        return Err(cli_msg::server_port_web_conflict_error(
            web_config.web_console_port,
            web_config.console_patch_port,
        ));
    }
    Ok([p1, p2])
}

/// Stage 1: Web Console TCP port & Auto-Broadcast settings (before entering dual-tab TUI).
pub async fn run_web_console_config_cli(
    terminal: &Terminal,
    config: &AppConfig,
    cancel: Option<&CancellationToken>,
) -> Result<WebConsoleSetupConfig, PromptError> {
    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        cli_msg::WEB_CONSOLE_SECTION_HEADER
    ));

    let web_port_prompt: String = format!(
        "{} * {} {}[Enter={}]{}: ",
        colors::LIGHT_WHITE,
        cli_msg::WEB_PORT_PROMPT,
        colors::LIGHT_BLACK,
        config.web_console_port,
        colors::LIGHT_WHITE,
    );
    let web_console_port: u16 = loop {
        let raw_web: String = terminal.ask(&web_port_prompt, cancel).await?;
        match validate_web_port(&raw_web, config.web_console_port, config.console_patch_port) {
            Ok(port) => break port,
            Err(err_msg) => terminal.println(&format!("{} * {err_msg}", colors::RED)),
        }
    };

    let default_broadcast_num: u8 = u8::from(config.auto_broadcast_default);
    let broadcast_prompt: String = format!(
        "{} * {}({}{}{}) {}[Enter={default_broadcast_num}]{}: ",
        colors::LIGHT_WHITE,
        cli_msg::AUTO_BROADCAST_PROMPT_PREFIX,
        colors::LIGHT_GREEN,
        cli_msg::AUTO_BROADCAST_CHOICES,
        colors::LIGHT_WHITE,
        colors::LIGHT_BLACK,
        colors::LIGHT_WHITE,
    );
    let auto_broadcast: bool = loop {
        let raw: String = terminal.ask(&broadcast_prompt, cancel).await?;
        match raw.as_str() {
            "" => break config.auto_broadcast_default,
            "1" => break true,
            "0" => break false,
            _ => terminal.println(&format!(
                "{} * {}",
                colors::RED,
                cli_msg::INVALID_CHOICE_RETRY
            )),
        }
    };

    Ok(WebConsoleSetupConfig {
        web_console_port,
        console_patch_port: config.console_patch_port,
        auto_broadcast,
    })
}

/// Stage 2: Game server alternating ports, map, match settings, and MOD selection (inside Tab 2).
pub async fn run_server_setup_cli(
    terminal: &Terminal,
    config: &AppConfig,
    web_config: WebConsoleSetupConfig,
    cancel: &CancellationToken,
) -> Result<Option<LaunchPlan>, PromptError> {
    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        cli_msg::SERVER_SETUP_SECTION_HEADER
    ));

    let p1_prompt: String = format!(
        "{} * {}({}1/2{}) {}[Enter={}]{}: ",
        colors::LIGHT_WHITE,
        cli_msg::SERVER_PORT_PROMPT_PREFIX,
        colors::LIGHT_GREEN,
        colors::LIGHT_WHITE,
        colors::LIGHT_BLACK,
        config.server_ports[0],
        colors::LIGHT_WHITE,
    );
    let p2_prompt: String = format!(
        "{} * {}({}2/2{}) {}[Enter={}]{}: ",
        colors::LIGHT_WHITE,
        cli_msg::SERVER_PORT_PROMPT_PREFIX,
        colors::LIGHT_GREEN,
        colors::LIGHT_WHITE,
        colors::LIGHT_BLACK,
        config.server_ports[1],
        colors::LIGHT_WHITE,
    );

    let server_ports: [u16; 2] = loop {
        let raw1: String = match terminal.ask(&p1_prompt, Some(cancel)).await {
            Ok(v) => v,
            Err(PromptError::Aborted) => return Ok(None),
            Err(e) => return Err(e),
        };
        let raw2: String = match terminal.ask(&p2_prompt, Some(cancel)).await {
            Ok(v) => v,
            Err(PromptError::Aborted) => return Ok(None),
            Err(e) => return Err(e),
        };
        match validate_server_ports(&raw1, &raw2, config.server_ports, web_config) {
            Ok(ports) => break ports,
            Err(err_msg) => terminal.println(&format!("{} * {err_msg}", colors::RED)),
        }
    };

    // Rotate clean before match/MOD configuration so Tab 2 retains only this round's config & injection logs
    terminal.reset_server_tab_round();
    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        cli_msg::SERVER_SETUP_SECTION_HEADER
    ));
    terminal.println(&format!(
        "{} * {} {}{} <-> {}",
        colors::LIGHT_WHITE,
        cli_msg::SERVER_PORTS_SELECTED_LABEL,
        colors::LIGHT_GREEN,
        server_ports[0],
        server_ports[1]
    ));

    let match_config: MatchConfig =
        match prompt_match_config(terminal, &ServerSettings::default(), Some(cancel)).await {
            Ok(cfg) => cfg,
            Err(PromptError::Aborted) => return Ok(None),
            Err(e) => return Err(e),
        };

    Ok(Some(LaunchPlan::from_parts(
        web_config,
        server_ports,
        match_config,
    )))
}

/// Post-match `R` reconfiguration wizard (preserves Web Console and alternating server ports).
pub async fn run_reconfigure_match_cli(
    terminal: &Terminal,
    current_plan: &LaunchPlan,
    cancel: &CancellationToken,
) -> Option<LaunchPlan> {
    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        cli_msg::RECONFIGURE_SECTION_HEADER
    ));
    terminal.println(&format!(
        "{} * {}",
        colors::LIGHT_BLACK,
        cli_msg::reconfigure_preserved_notice(
            current_plan.server_ports[0],
            current_plan.server_ports[1]
        )
    ));

    match prompt_match_config(terminal, &current_plan.settings, Some(cancel)).await {
        Ok(match_config) => Some(current_plan.with_match_config(match_config)),
        Err(PromptError::Aborted) => None,
        Err(PromptError::Io(msg)) => {
            if cancel.is_cancelled() {
                return None;
            }
            terminal.println(&format!(
                "{} * {}",
                colors::RED,
                cli_msg::reconfigure_interrupted_notice(&msg)
            ));
            Some(current_plan.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_web_and_server_port_collisions() {
        assert_eq!(validate_web_port("", 7891, 7893), Ok(7891));
        assert!(validate_web_port("7893", 7891, 7893).is_err());
        assert!(validate_web_port("80", 7891, 7893).is_err());
        assert!(validate_web_port("70000", 7891, 7893).is_err());

        let web_cfg = WebConsoleSetupConfig {
            web_console_port: 7891,
            console_patch_port: 7893,
            auto_broadcast: true,
        };
        assert_eq!(
            validate_server_ports("", "", [7895, 7897], web_cfg),
            Ok([7895, 7897])
        );
        assert!(validate_server_ports("7895", "7895", [7895, 7897], web_cfg).is_err());
        assert!(validate_server_ports("7891", "7897", [7895, 7897], web_cfg).is_err());
        assert!(validate_server_ports("7895", "7893", [7895, 7897], web_cfg).is_err());
        assert!(validate_server_ports("1023", "7897", [7895, 7897], web_cfg).is_err());
    }
}
