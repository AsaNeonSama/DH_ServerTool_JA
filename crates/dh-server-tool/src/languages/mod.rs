//! Code-driven i18n lookup and rendering layer.
//!
//! Business logic passes strongly-typed message codes and raw parameters (`ApiCode`,
//! `WebLogEvent`, `BroadcastEvent`, `AppMsg`, `CliMsg`, `ServerMsg`, `TerminalMsg`,
//! `ModLabelCode`) rather than embedding localized strings directly.

pub mod ja;

use crate::structs::languages::{
    ApiCode, AppMsg, AppMsgCode, BroadcastEvent, CliErrorCode, CliMsg, CliMsgCode, Locale,
    LogCategoryCode, ModLabelCode, ResultBadgeCode, ServerMsg, ServerMsgCode, TerminalMsg,
    TerminalMsgCode, WebLogDetailCode,
};

#[must_use]
pub fn active_locale() -> Locale {
    Locale::Ja
}

#[must_use]
pub fn app_text(code: AppMsgCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::app::text(code),
    }
}

#[must_use]
pub fn render_app(msg: &AppMsg<'_>) -> String {
    match active_locale() {
        Locale::Ja => ja::app::render(msg),
    }
}

#[must_use]
pub fn cli_text(code: CliMsgCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::cli::text(code),
    }
}

#[must_use]
pub fn render_cli(msg: &CliMsg<'_>) -> String {
    match active_locale() {
        Locale::Ja => ja::cli::render(msg),
    }
}

#[must_use]
pub fn render_cli_error(err: CliErrorCode) -> String {
    match active_locale() {
        Locale::Ja => ja::cli::render_error(err),
    }
}

#[must_use]
pub fn mod_label(code: ModLabelCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::mods::label(code),
    }
}

#[must_use]
pub fn server_text(code: ServerMsgCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::server::text(code),
    }
}

#[must_use]
pub fn render_server(msg: &ServerMsg<'_>) -> String {
    match active_locale() {
        Locale::Ja => ja::server::render(msg),
    }
}

#[must_use]
pub fn terminal_text(code: TerminalMsgCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::terminal::text(code),
    }
}

#[must_use]
pub fn render_terminal(msg: &TerminalMsg<'_>) -> String {
    match active_locale() {
        Locale::Ja => ja::terminal::render(msg),
    }
}

#[must_use]
pub fn api_text(code: ApiCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::web_console::api_text(code),
    }
}

#[must_use]
pub fn log_category_text(code: LogCategoryCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::web_console::log_category_text(code),
    }
}

#[must_use]
pub fn render_result_badge(code: ResultBadgeCode, suffix: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::web_console::render_result_badge(code, suffix),
    }
}

#[must_use]
pub fn web_detail_text(code: WebLogDetailCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::web_console::detail_text(code),
    }
}

#[must_use]
pub fn player_role_name(role_key: &str) -> Option<&'static str> {
    match active_locale() {
        Locale::Ja => ja::web_console::player_role_name(role_key),
    }
}

#[must_use]
pub fn actor_display_name(key: &str) -> Option<&'static str> {
    match active_locale() {
        Locale::Ja => ja::web_console::actor_display_name(key),
    }
}

#[must_use]
pub fn render_broadcast(event: &BroadcastEvent<'_>) -> String {
    match active_locale() {
        Locale::Ja => ja::web_console::render_broadcast(event),
    }
}

#[must_use]
pub fn render_unauthenticated_rejected(client_ip: &str, pathname: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::web_console::unauthenticated_rejected(client_ip, pathname),
    }
}

#[must_use]
pub fn render_invalid_request_body(client_ip: &str, pathname: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::web_console::invalid_request_body(client_ip, pathname),
    }
}

#[must_use]
pub fn render_decrypt_failed(client_ip: &str, pathname: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::web_console::decrypt_failed(client_ip, pathname),
    }
}

#[must_use]
pub fn render_invalid_json_params(client_ip: &str, pathname: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::web_console::invalid_json_params(client_ip, pathname),
    }
}

#[must_use]
pub fn render_login_failed_invalid_body(client_ip: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::web_console::login_failed_invalid_body(client_ip),
    }
}

#[must_use]
pub fn render_login_failed_psk_mismatch(client_ip: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::web_console::login_failed_psk_mismatch(client_ip),
    }
}

#[must_use]
pub fn render_login_failed_challenge_invalid(client_ip: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::web_console::login_failed_challenge_invalid(client_ip),
    }
}

#[must_use]
pub fn render_login_succeeded(client_ip: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::web_console::login_succeeded(client_ip),
    }
}

#[must_use]
pub fn render_startup_detail(web_port: u16, patch_port: u16, auto_broadcast: bool) -> String {
    let toggle_code: TerminalMsgCode = if auto_broadcast {
        TerminalMsgCode::ToggleOn
    } else {
        TerminalMsgCode::ToggleOff
    };
    let toggle_text: &str = terminal_text(toggle_code);
    match active_locale() {
        Locale::Ja => ja::web_console::startup_detail(web_port, patch_port, toggle_text),
    }
}

#[must_use]
pub fn render_copy_log(is_public: bool, success: bool) -> String {
    let label_code: TerminalMsgCode = if is_public {
        TerminalMsgCode::PublicUrlShortLabel
    } else {
        TerminalMsgCode::LocalUrlShortLabel
    };
    let label: &str = terminal_text(label_code);
    match active_locale() {
        Locale::Ja => ja::web_console::copy_log(label, success),
    }
}

#[must_use]
pub fn quote_message(message: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::web_console::quote_message(message),
    }
}
