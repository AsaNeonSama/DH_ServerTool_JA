use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tokio::task::JoinError;
use tokio_util::sync::CancellationToken;

use dh_server_tool::cli::{
    run_server_setup_cli, run_web_console_config_cli, LaunchPlan, WebConsoleSetupConfig,
};
use dh_server_tool::colors;
use dh_server_tool::config::{
    ensure_server_working_dir, load_or_create_config, AppConfig, ConfigLoadOutcome, LoadedConfig,
    CONFIG_RELATIVE_PATH,
};
use dh_server_tool::format::format_seconds;
use dh_server_tool::languages;
use dh_server_tool::mods::{PatchScript, DEFAULT_PATCH_SCRIPTS};
use dh_server_tool::network::{fetch_public_ip, PublicIpResult, DEFAULT_PUBLIC_IP_TIMEOUT_MS};
use dh_server_tool::server::{run_server_supervisor, LEGACY_GADGET_DLL_FILENAME};
use dh_server_tool::structs::languages::{AppMsg, AppMsgCode, ConfigSummaryParams};
use dh_server_tool::terminal::{
    install_panic_hook, restore_terminal_emergency, PromptError, Terminal, TerminalTabId,
};
use dh_server_tool::web_console::{
    format_non_tty_startup_banner, start_web_console, WebConsoleHandle, WebConsoleOptions,
};

fn print_launch_summary(terminal: &Terminal, plan: &LaunchPlan) {
    let default_mod_names: String = DEFAULT_PATCH_SCRIPTS
        .iter()
        .map(|patch: &PatchScript| languages::mod_label(patch.label_code))
        .collect::<Vec<&str>>()
        .join(", ");
    let content_mod_names: String = if plan.selected_content_option_labels.is_empty() {
        languages::app_text(AppMsgCode::NoneLabel).to_string()
    } else {
        plan.selected_content_option_labels.join(", ")
    };
    let selected_mod_names: String = if plan.selected_option_labels.is_empty() {
        languages::app_text(AppMsgCode::NoneLabel).to_string()
    } else {
        plan.selected_option_labels.join(", ")
    };

    terminal.println_empty();
    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        languages::app_text(AppMsgCode::SummaryHeader)
    ));
    terminal.println(&format!(
        "{} * {} {}{} ({})",
        colors::LIGHT_WHITE,
        languages::app_text(AppMsgCode::SummaryMapLabel),
        colors::LIGHT_GREEN,
        languages::cli_text(plan.map_label_code),
        plan.map_id
    ));
    if plan.switch_port_on_restart {
        terminal.println(&format!(
            "{} * {} {}{} <-> {} {}{}",
            colors::LIGHT_WHITE,
            languages::app_text(AppMsgCode::SummaryServerPortLabel),
            colors::LIGHT_GREEN,
            plan.server_ports[0],
            plan.server_ports[1],
            colors::LIGHT_BLACK,
            languages::app_text(AppMsgCode::SummaryServerPortSwitchOnSuffix),
        ));
    } else {
        terminal.println(&format!(
            "{} * {} {}{} {}{}",
            colors::LIGHT_WHITE,
            languages::app_text(AppMsgCode::SummaryServerPortLabel),
            colors::LIGHT_GREEN,
            plan.server_ports[0],
            colors::LIGHT_BLACK,
            languages::app_text(AppMsgCode::SummaryServerPortFixedSuffix),
        ));
    }
    terminal.println(&format!(
        "{} * {} {}{} {}{}",
        colors::LIGHT_WHITE,
        languages::app_text(AppMsgCode::SummaryWebConsoleLabel),
        colors::LIGHT_GREEN,
        plan.web_console_port,
        colors::LIGHT_BLACK,
        languages::render_app(&AppMsg::SummaryInternalPatchPort {
            port: plan.console_patch_port,
        })
    ));
    terminal.println(&format!(
        "{} * {} {}{default_mod_names}",
        colors::LIGHT_WHITE,
        languages::app_text(AppMsgCode::SummaryDefaultModsLabel),
        colors::CYAN,
    ));
    terminal.println(&format!(
        "{} * {} {}{content_mod_names}",
        colors::LIGHT_WHITE,
        languages::app_text(AppMsgCode::SummaryContentModsLabel),
        colors::LIGHT_MAGENTA,
    ));
    terminal.println(&format!(
        "{} * {} {}{selected_mod_names}",
        colors::LIGHT_WHITE,
        languages::app_text(AppMsgCode::SummarySelectedModsLabel),
        colors::CYAN,
    ));
    terminal.println(&format!(
        "{} {}",
        colors::LIGHT_CYAN,
        languages::app_text(AppMsgCode::SummaryFooter)
    ));
}

async fn run_app() -> ExitCode {
    install_panic_hook();

    let terminal: Terminal = Terminal::new();
    terminal.setup(colors::APP_CONSOLE_TITLE);

    // Best-effort cleanup of stale per-round Gadget staging directories from any previous crash.
    dh_gadget_injector::sweep_default_stale_run_dirs();

    // Author & Antigravity branding banner
    terminal.println(&format!(
        "{} * {} {}{}  {}{} {}{} {}{}",
        colors::LIGHT_CYAN,
        languages::app_text(AppMsgCode::AuthorLabel),
        colors::LIGHT_WHITE,
        languages::app_text(AppMsgCode::AuthorName),
        colors::LIGHT_YELLOW,
        languages::app_text(AppMsgCode::BrandBuild),
        colors::LIGHT_GREEN,
        languages::app_text(AppMsgCode::BrandWith),
        colors::LIGHT_BLUE,
        languages::app_text(AppMsgCode::BrandAntigravity),
    ));
    terminal.println(&format!(
        "{} * {} {}{}",
        colors::LIGHT_BLUE,
        languages::app_text(AppMsgCode::UsageNoticeLabel),
        colors::LIGHT_BLACK,
        colors::APP_LICENSE_URL,
    ));

    // Ensure working directory contains the server binary (relative paths only)
    let default_cfg: AppConfig = AppConfig::default();
    if !ensure_server_working_dir(&default_cfg.shipping_executable) {
        terminal.println(&format!(
            "{} * {}",
            colors::RED,
            languages::app_text(AppMsgCode::MissingServerDirError)
        ));
        terminal.wait_for_exit_key().await;
        return ExitCode::FAILURE;
    }

    // Fetch public IP concurrently with loading/creating config.json
    let (ip_result, config_join): (PublicIpResult, Result<LoadedConfig, JoinError>) = tokio::join!(
        fetch_public_ip(DEFAULT_PUBLIC_IP_TIMEOUT_MS),
        tokio::task::spawn_blocking(load_or_create_config),
    );

    let config: AppConfig = match config_join {
        Ok(loaded) => {
            match &loaded.outcome {
                ConfigLoadOutcome::CreatedDefault => {
                    terminal.println(&format!(
                        "{} * {}",
                        colors::LIGHT_CYAN,
                        languages::render_app(&AppMsg::ConfigCreated {
                            relative_path: CONFIG_RELATIVE_PATH,
                        })
                    ));
                }
                ConfigLoadOutcome::LoadFailed(err) => {
                    terminal.println(&format!(
                        "{} * {}",
                        colors::RED,
                        languages::render_app(&AppMsg::ConfigLoadFailed { error_text: err })
                    ));
                }
                ConfigLoadOutcome::Loaded | ConfigLoadOutcome::CreateFailed => {}
            }
            loaded.config
        }
        Err(err) => {
            let err_str: String = err.to_string();
            terminal.println(&format!(
                "{} * {}",
                colors::RED,
                languages::render_app(&AppMsg::ConfigLoadFailed {
                    error_text: &err_str,
                })
            ));
            AppConfig::default()
        }
    };

    if let Some(ref ip) = ip_result.ip {
        terminal.println(&format!(
            "{} * {} {}{ip}",
            colors::LIGHT_CYAN,
            languages::app_text(AppMsgCode::PublicIpLabel),
            colors::LIGHT_GREEN,
        ));
    } else {
        let err_text: &str = ip_result.error.as_deref().unwrap_or("unknown error");
        terminal.println(&format!(
            "{} * {}",
            colors::RED,
            languages::render_app(&AppMsg::PublicIpFetchFailed {
                error_text: err_text,
            })
        ));
    }

    let shipping_path: &Path = Path::new(&config.shipping_executable);
    if !shipping_path.exists() {
        terminal.println(&format!(
            "{} * {}",
            colors::RED,
            languages::render_app(&AppMsg::MissingShippingExecutable {
                path: &config.shipping_executable,
            })
        ));
        terminal.println(&format!(
            "{} * {}",
            colors::RED,
            languages::app_text(AppMsgCode::MissingServerDirError)
        ));
        terminal.wait_for_exit_key().await;
        return ExitCode::FAILURE;
    }

    // Warn if legacy frida.dll residue exists next to the game server executable
    let legacy_frida_dll: PathBuf = shipping_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(LEGACY_GADGET_DLL_FILENAME);
    if legacy_frida_dll.exists() {
        let legacy_path_str: Cow<'_, str> = legacy_frida_dll.to_string_lossy();
        terminal.println(&format!(
            "{} * {}",
            colors::LIGHT_YELLOW,
            languages::render_app(&AppMsg::LegacyFridaDllDetected {
                file_name: LEGACY_GADGET_DLL_FILENAME,
                full_path: &legacy_path_str,
            })
        ));
        terminal.println(&format!(
            "{} * {}",
            colors::LIGHT_YELLOW,
            languages::render_app(&AppMsg::LegacyFridaDllAdvice {
                process_name: &config.shipping_process_name,
            })
        ));
    }

    let interval_text: String = format_seconds(config.check_interval_ms);
    terminal.println(&format!(
        "{} * {}",
        colors::LIGHT_BLACK,
        languages::render_app(&AppMsg::ConfigSummary(ConfigSummaryParams {
            server_port_1: config.server_ports[0],
            server_port_2: config.server_ports[1],
            switch_port_on_restart: config.switch_port_on_restart,
            web_console_port: config.web_console_port,
            console_patch_port: config.console_patch_port,
            check_interval_text: &interval_text,
            restart_wait_seconds: config.restart_wait_seconds,
        }))
    ));
    terminal.println_empty();

    // Shared graceful shutdown token triggered by OS Ctrl+C or raw-mode TUI Ctrl+C.
    let shutdown_token: CancellationToken = CancellationToken::new();
    let ctrl_c_token: CancellationToken = shutdown_token.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            ctrl_c_token.cancel();
        }
    });

    // Stage 1: Web Console TCP port & Auto-Broadcast settings (plain terminal)
    let web_config: WebConsoleSetupConfig =
        match run_web_console_config_cli(&terminal, &config, Some(&shutdown_token)).await {
            Ok(cfg) => cfg,
            Err(PromptError::Aborted) => {
                terminal.println(&format!(
                    "{} * {}",
                    colors::LIGHT_GREEN,
                    languages::app_text(AppMsgCode::ExitedCleanly)
                ));
                return ExitCode::SUCCESS;
            }
            Err(PromptError::Io(msg)) => {
                terminal.println(&format!(
                    "{} * {}\n{msg}",
                    colors::RED,
                    languages::app_text(AppMsgCode::UnexpectedErrorHeader),
                ));
                terminal.wait_for_exit_key().await;
                return ExitCode::FAILURE;
            }
        };

    // Start integrated Web Console IMMEDIATELY
    let web_console: WebConsoleHandle = match start_web_console(WebConsoleOptions {
        web_port: web_config.web_console_port,
        console_patch_port: web_config.console_patch_port,
        auto_broadcast: web_config.auto_broadcast,
        public_ip: ip_result.ip,
        log_sink: terminal.web_log_sink(),
    })
    .await
    {
        Ok(handle) => handle,
        Err(err) => {
            let err_str: String = err.to_string();
            terminal.println(&format!(
                "{} * {}",
                colors::RED,
                languages::render_app(&AppMsg::WebConsoleStartFailed {
                    port: web_config.web_console_port,
                    error_text: &err_str,
                })
            ));
            terminal.wait_for_exit_key().await;
            return ExitCode::FAILURE;
        }
    };

    // Enter Dual-Tab Alternate Screen TUI (default focus: Tab 2 Server) if interactive TTY
    let entered_tui: bool = if terminal.is_interactive_tty() {
        terminal
            .enter_tabbed_mode(
                web_console.display_info.clone(),
                TerminalTabId::Server,
                shutdown_token.clone(),
            )
            .unwrap_or(false)
    } else {
        false
    };

    if !entered_tui {
        for line in format_non_tty_startup_banner(&web_console.display_info) {
            terminal.println(&line);
        }
    }

    // Stage 2: Server alternating ports, map, match parameters, and MOD selection (Tab 2)
    let mut exit_code: ExitCode = ExitCode::SUCCESS;
    match run_server_setup_cli(&terminal, &config, web_config, &shutdown_token).await {
        Ok(Some(plan)) if !shutdown_token.is_cancelled() => {
            print_launch_summary(&terminal, &plan);
            run_server_supervisor(&terminal, plan, &config, &shutdown_token).await;
        }
        Ok(_) | Err(PromptError::Aborted) => {}
        Err(PromptError::Io(msg)) => {
            terminal.exit_tabbed_mode().await;
            terminal.println(&format!(
                "{} * {}\n{msg}",
                colors::RED,
                languages::app_text(AppMsgCode::UnexpectedErrorHeader),
            ));
            exit_code = ExitCode::FAILURE;
        }
    }

    // Guaranteed clean teardown of TUI + Web Console
    terminal.exit_tabbed_mode().await;
    if shutdown_token.is_cancelled() {
        terminal.println(&format!(
            "{} * {}",
            colors::LIGHT_YELLOW,
            languages::app_text(AppMsgCode::ShutdownSignalReceived)
        ));
    }
    web_console.shutdown().await;
    restore_terminal_emergency();

    if exit_code == ExitCode::SUCCESS {
        terminal.println(&format!(
            "{} * {}",
            colors::LIGHT_GREEN,
            languages::app_text(AppMsgCode::ExitedCleanly)
        ));
    } else {
        terminal.wait_for_exit_key().await;
    }

    exit_code
}

#[tokio::main]
async fn main() -> ExitCode {
    run_app().await
}
