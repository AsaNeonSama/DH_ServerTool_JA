use tokio_util::sync::CancellationToken;

use crate::cli::match_setup::{prompt_match_config, MatchConfig, ServerSettings};
use crate::colors;
use crate::config::{is_valid_port_number, AppConfig, MAX_VALID_PORT, MIN_VALID_PORT};
use crate::languages;
pub use crate::structs::cli::{LaunchPlan, WebConsoleSetupConfig};
use crate::structs::languages::{CliErrorCode, CliMsg, CliMsgCode};
use crate::terminal::{PromptError, Terminal};

impl LaunchPlan {
    #[must_use]
    pub fn from_parts(
        web_config: WebConsoleSetupConfig,
        server_ports: [u16; 2],
        switch_port_on_restart: bool,
        match_config: MatchConfig,
    ) -> Self {
        Self {
            web_console_port: web_config.web_console_port,
            console_patch_port: web_config.console_patch_port,
            auto_broadcast: web_config.auto_broadcast,
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

    #[must_use]
    pub fn with_match_config(&self, match_config: MatchConfig) -> Self {
        Self {
            web_console_port: self.web_console_port,
            console_patch_port: self.console_patch_port,
            auto_broadcast: self.auto_broadcast,
            server_ports: self.server_ports,
            switch_port_on_restart: self.switch_port_on_restart,
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
pub fn parse_toggle_input(raw: &str, default_value: bool) -> Option<bool> {
    match raw.trim() {
        "" => Some(default_value),
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
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

pub fn validate_web_port(
    raw: &str,
    fallback: u16,
    console_patch_port: u16,
) -> Result<u16, CliErrorCode> {
    let Some(parsed): Option<u16> = parse_port_input(raw, fallback) else {
        return Err(CliErrorCode::PortOutOfRange {
            min: MIN_VALID_PORT,
            max: MAX_VALID_PORT,
        });
    };
    if parsed == console_patch_port {
        return Err(CliErrorCode::WebPortConflictsWithPatch {
            patch_port: console_patch_port,
        });
    }
    Ok(parsed)
}

pub fn validate_single_server_port(
    raw: &str,
    default_port: u16,
    web_config: WebConsoleSetupConfig,
) -> Result<u16, CliErrorCode> {
    let Some(port): Option<u16> = parse_port_input(raw, default_port) else {
        return Err(CliErrorCode::PortOutOfRange {
            min: MIN_VALID_PORT,
            max: MAX_VALID_PORT,
        });
    };
    if port == web_config.web_console_port || port == web_config.console_patch_port {
        return Err(CliErrorCode::ServerPortConflictsWithWeb {
            web_port: web_config.web_console_port,
            patch_port: web_config.console_patch_port,
        });
    }
    Ok(port)
}

pub fn validate_server_ports(
    raw1: &str,
    raw2: &str,
    default_ports: [u16; 2],
    web_config: WebConsoleSetupConfig,
) -> Result<[u16; 2], CliErrorCode> {
    let (Some(p1), Some(p2)): (Option<u16>, Option<u16>) = (
        parse_port_input(raw1, default_ports[0]),
        parse_port_input(raw2, default_ports[1]),
    ) else {
        return Err(CliErrorCode::PortOutOfRange {
            min: MIN_VALID_PORT,
            max: MAX_VALID_PORT,
        });
    };
    if p1 == p2 {
        return Err(CliErrorCode::ServerPortsDuplicate);
    }
    if p1 == web_config.web_console_port
        || p2 == web_config.web_console_port
        || p1 == web_config.console_patch_port
        || p2 == web_config.console_patch_port
    {
        return Err(CliErrorCode::ServerPortConflictsWithWeb {
            web_port: web_config.web_console_port,
            patch_port: web_config.console_patch_port,
        });
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
        languages::cli_text(CliMsgCode::WebConsoleSectionHeader)
    ));

    let web_port_prompt: String = format!(
        "{} * {} {}[Enter={}]{}: ",
        colors::LIGHT_WHITE,
        languages::cli_text(CliMsgCode::WebPortPrompt),
        colors::LIGHT_BLACK,
        config.web_console_port,
        colors::LIGHT_WHITE,
    );
    let web_console_port: u16 = loop {
        let raw_web: String = terminal.ask(&web_port_prompt, cancel).await?;
        match validate_web_port(&raw_web, config.web_console_port, config.console_patch_port) {
            Ok(port) => break port,
            Err(err_code) => terminal.println(&format!(
                "{} * {}",
                colors::RED,
                languages::render_cli_error(err_code)
            )),
        }
    };

    let default_broadcast_num: u8 = u8::from(config.auto_broadcast_default);
    let broadcast_prompt: String = format!(
        "{} * {}({}{}{}) {}[Enter={default_broadcast_num}]{}: ",
        colors::LIGHT_WHITE,
        languages::cli_text(CliMsgCode::AutoBroadcastPromptPrefix),
        colors::LIGHT_GREEN,
        languages::cli_text(CliMsgCode::AutoBroadcastChoices),
        colors::LIGHT_WHITE,
        colors::LIGHT_BLACK,
        colors::LIGHT_WHITE,
    );
    let auto_broadcast: bool = loop {
        let raw: String = terminal.ask(&broadcast_prompt, cancel).await?;
        if let Some(enabled) = parse_toggle_input(&raw, config.auto_broadcast_default) {
            break enabled;
        }
        terminal.println(&format!(
            "{} * {}",
            colors::RED,
            languages::cli_text(CliMsgCode::InvalidChoiceRetry)
        ));
    };

    Ok(WebConsoleSetupConfig {
        web_console_port,
        console_patch_port: config.console_patch_port,
        auto_broadcast,
    })
}

/// Stage 2: Game server port switching mode, port(s), map, match settings, and MOD selection (inside Tab 2).
pub async fn run_server_setup_cli(
    terminal: &Terminal,
    config: &AppConfig,
    web_config: WebConsoleSetupConfig,
    cancel: &CancellationToken,
) -> Result<Option<LaunchPlan>, PromptError> {
    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        languages::cli_text(CliMsgCode::ServerSetupSectionHeader)
    ));

    let default_switch_num: u8 = u8::from(config.switch_port_on_restart);
    let switch_prompt: String = format!(
        "{} * {}({}{}{}) {}[Enter={default_switch_num}]{}: ",
        colors::LIGHT_WHITE,
        languages::cli_text(CliMsgCode::SwitchPortOnRestartPromptPrefix),
        colors::LIGHT_GREEN,
        languages::cli_text(CliMsgCode::SwitchPortOnRestartChoices),
        colors::LIGHT_WHITE,
        colors::LIGHT_BLACK,
        colors::LIGHT_WHITE,
    );
    let switch_port_on_restart: bool = loop {
        let raw_switch: String = match terminal.ask(&switch_prompt, Some(cancel)).await {
            Ok(v) => v,
            Err(PromptError::Aborted) => return Ok(None),
            Err(e) => return Err(e),
        };
        if let Some(enabled) = parse_toggle_input(&raw_switch, config.switch_port_on_restart) {
            break enabled;
        }
        terminal.println(&format!(
            "{} * {}",
            colors::RED,
            languages::cli_text(CliMsgCode::InvalidChoiceRetry)
        ));
    };

    let server_ports: [u16; 2] = if switch_port_on_restart {
        let p1_prompt: String = format!(
            "{} * {}({}1/2{}) {}[Enter={}]{}: ",
            colors::LIGHT_WHITE,
            languages::cli_text(CliMsgCode::ServerPortPromptPrefix),
            colors::LIGHT_GREEN,
            colors::LIGHT_WHITE,
            colors::LIGHT_BLACK,
            config.server_ports[0],
            colors::LIGHT_WHITE,
        );
        let p2_prompt: String = format!(
            "{} * {}({}2/2{}) {}[Enter={}]{}: ",
            colors::LIGHT_WHITE,
            languages::cli_text(CliMsgCode::ServerPortPromptPrefix),
            colors::LIGHT_GREEN,
            colors::LIGHT_WHITE,
            colors::LIGHT_BLACK,
            config.server_ports[1],
            colors::LIGHT_WHITE,
        );

        loop {
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
                Err(err_code) => terminal.println(&format!(
                    "{} * {}",
                    colors::RED,
                    languages::render_cli_error(err_code)
                )),
            }
        }
    } else {
        let single_port_prompt: String = format!(
            "{} * {} {}[Enter={}]{}: ",
            colors::LIGHT_WHITE,
            languages::cli_text(CliMsgCode::ServerPortPromptPrefix),
            colors::LIGHT_BLACK,
            config.server_ports[0],
            colors::LIGHT_WHITE,
        );

        loop {
            let raw_port: String = match terminal.ask(&single_port_prompt, Some(cancel)).await {
                Ok(v) => v,
                Err(PromptError::Aborted) => return Ok(None),
                Err(e) => return Err(e),
            };
            match validate_single_server_port(&raw_port, config.server_ports[0], web_config) {
                Ok(port) => break [port, port],
                Err(err_code) => terminal.println(&format!(
                    "{} * {}",
                    colors::RED,
                    languages::render_cli_error(err_code)
                )),
            }
        }
    };

    // Rotate clean before match/MOD configuration so Tab 2 retains only this round's config & injection logs
    terminal.reset_server_tab_round();
    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        languages::cli_text(CliMsgCode::ServerSetupSectionHeader)
    ));
    if switch_port_on_restart {
        terminal.println(&format!(
            "{} * {} {}{} <-> {} {}{}",
            colors::LIGHT_WHITE,
            languages::cli_text(CliMsgCode::ServerPortsSelectedLabel),
            colors::LIGHT_GREEN,
            server_ports[0],
            server_ports[1],
            colors::LIGHT_BLACK,
            languages::cli_text(CliMsgCode::ServerPortSwitchBadge),
        ));
    } else {
        terminal.println(&format!(
            "{} * {} {}{} {}{}",
            colors::LIGHT_WHITE,
            languages::cli_text(CliMsgCode::ServerPortsSelectedLabel),
            colors::LIGHT_GREEN,
            server_ports[0],
            colors::LIGHT_BLACK,
            languages::cli_text(CliMsgCode::ServerPortFixedBadge),
        ));
    }

    let match_config: MatchConfig =
        match prompt_match_config(terminal, &ServerSettings::default(), Some(cancel)).await {
            Ok(cfg) => cfg,
            Err(PromptError::Aborted) => return Ok(None),
            Err(e) => return Err(e),
        };

    Ok(Some(LaunchPlan::from_parts(
        web_config,
        server_ports,
        switch_port_on_restart,
        match_config,
    )))
}

/// Post-match `R` reconfiguration wizard (preserves Web Console and server port settings).
pub async fn run_reconfigure_match_cli(
    terminal: &Terminal,
    current_plan: &LaunchPlan,
    cancel: &CancellationToken,
) -> Option<LaunchPlan> {
    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        languages::cli_text(CliMsgCode::ReconfigureSectionHeader)
    ));
    terminal.println(&format!(
        "{} * {}",
        colors::LIGHT_BLACK,
        languages::render_cli(&CliMsg::ReconfigurePreservedNotice {
            port_1: current_plan.server_ports[0],
            port_2: current_plan.server_ports[1],
            switch_port_on_restart: current_plan.switch_port_on_restart,
        })
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
                languages::render_cli(&CliMsg::ReconfigureInterruptedNotice { error_text: &msg })
            ));
            Some(current_plan.clone())
        }
    }
}
