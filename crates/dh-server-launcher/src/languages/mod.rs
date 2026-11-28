//! Code-driven i18n lookup and rendering layer.

pub mod ja;

use crate::structs::languages::{
    ApiCode, AppMsg, BroadcastEvent, ConsoleLogDetailCode, GuiMsg, GuiMsgCode, Locale,
    LogCategoryCode, ModLabelCode, ResultBadgeCode, ServerMsg, ServerMsgCode,
};

#[must_use]
pub fn active_locale() -> Locale {
    Locale::Ja
}

#[must_use]
pub fn render_app(msg: &AppMsg<'_>) -> String {
    match active_locale() {
        Locale::Ja => ja::app::render(msg),
    }
}

#[must_use]
pub fn mod_label(code: ModLabelCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::mods::label(code),
    }
}

#[must_use]
pub fn mod_description(code: ModLabelCode) -> Option<&'static str> {
    match active_locale() {
        Locale::Ja => ja::mods::description(code),
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
pub fn api_text(code: ApiCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::console::api_text(code),
    }
}

#[must_use]
pub fn log_category_text(code: LogCategoryCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::console::log_category_text(code),
    }
}

#[must_use]
pub fn render_result_badge(code: ResultBadgeCode, suffix: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::console::render_result_badge(code, suffix),
    }
}

#[must_use]
pub fn console_detail_text(code: ConsoleLogDetailCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::console::detail_text(code),
    }
}

#[must_use]
pub fn player_role_name(role_key: &str) -> Option<&'static str> {
    match active_locale() {
        Locale::Ja => ja::console::player_role_name(role_key),
    }
}

#[must_use]
pub fn actor_display_name(key: &str) -> Option<&'static str> {
    match active_locale() {
        Locale::Ja => ja::console::actor_display_name(key),
    }
}

#[must_use]
pub fn render_broadcast(event: &BroadcastEvent<'_>) -> String {
    match active_locale() {
        Locale::Ja => ja::console::render_broadcast(event),
    }
}

#[must_use]
pub fn quote_message(message: &str) -> String {
    match active_locale() {
        Locale::Ja => ja::console::quote_message(message),
    }
}

#[must_use]
pub fn gui_text(code: GuiMsgCode) -> &'static str {
    match active_locale() {
        Locale::Ja => ja::gui::text(code),
    }
}

#[must_use]
pub fn render_gui(msg: &GuiMsg<'_>) -> String {
    match active_locale() {
        Locale::Ja => ja::gui::render(msg),
    }
}
