use std::sync::Arc;

use crate::colors;
use crate::consts::web_console::status_code;
pub use crate::consts::web_console::MAX_LOG_TEXT_CHARS;
use crate::format::{format_local_time, sanitize_log_text};
use crate::languages;
use crate::structs::languages::{LogCategoryCode, ResultBadgeCode, WebLogDetailCode, WebLogEvent};
pub use crate::structs::web_console::WebLogSink;

#[must_use]
pub fn noop_log_sink() -> WebLogSink {
    Arc::new(|_: String| {})
}

#[must_use]
pub fn normalize_client_ip(raw_ip: Option<&str>) -> String {
    let Some(raw): Option<&str> = raw_ip else {
        return "unknown".to_string();
    };
    let trimmed: &str = raw.trim();
    let stripped: &str = trimmed.strip_prefix("::ffff:").unwrap_or(trimmed);
    let normalized: &str = if stripped == "::1" {
        "127.0.0.1"
    } else {
        stripped
    };
    let safe: String = sanitize_log_text(normalized, 45);
    if safe.is_empty() {
        "unknown".to_string()
    } else {
        safe
    }
}

#[must_use]
pub fn format_result_badge(status: u16, err_detail: Option<&str>) -> String {
    if status == status_code::SUCCESS {
        return format!(
            "{}{}",
            colors::LIGHT_GREEN,
            languages::render_result_badge(ResultBadgeCode::Sent, "")
        );
    }
    if status == status_code::PARTIAL_SUCCESS {
        return format!(
            "{}{}",
            colors::LIGHT_YELLOW,
            languages::render_result_badge(ResultBadgeCode::PartialFailure, "")
        );
    }
    let suffix: String = match err_detail {
        Some(detail) if !detail.is_empty() => format!(": {}", sanitize_log_text(detail, 36)),
        _ => String::new(),
    };
    if status == status_code::SERVER_ERROR {
        format!(
            "{}{}",
            colors::RED,
            languages::render_result_badge(ResultBadgeCode::SendFailed, &suffix)
        )
    } else {
        format!(
            "{}{}",
            colors::RED,
            languages::render_result_badge(ResultBadgeCode::Invalid, &suffix)
        )
    }
}

#[must_use]
pub fn format_console_command_preview(command: &str) -> String {
    let trimmed: &str = command.trim();
    let (raw_cmd_name, cmd_arg): (&str, &str) = match trimmed.find(' ') {
        Some(idx) => (&trimmed[..idx], trimmed[idx + 1..].trim()),
        None => (trimmed, ""),
    };
    let cmd_name: String = raw_cmd_name.to_ascii_lowercase();
    if cmd_name == "summon" && !cmd_arg.is_empty() {
        if let Some(known_name) = languages::actor_display_name(cmd_arg) {
            return format!("summon {known_name}");
        }
    }
    let normalized: String = if cmd_arg.is_empty() {
        cmd_name
    } else {
        format!("{cmd_name} {cmd_arg}")
    };
    sanitize_log_text(&normalized, MAX_LOG_TEXT_CHARS)
}

fn sanitize_role_label(role_key: &str) -> String {
    let display: &str = languages::player_role_name(role_key).unwrap_or(role_key);
    sanitize_log_text(display, 24)
}

fn sanitize_item_label(item_key: &str) -> String {
    let display: &str = languages::actor_display_name(item_key).unwrap_or(item_key);
    sanitize_log_text(display, 40)
}

fn render_invalid_params(
    category: LogCategoryCode,
    client_ip: &str,
) -> (LogCategoryCode, &'static str, String) {
    let color: &'static str = if matches!(category, LogCategoryCode::Kick) {
        colors::LIGHT_RED
    } else {
        colors::LIGHT_YELLOW
    };
    let detail: String = format!(
        "{client_ip} | {} {}",
        languages::web_detail_text(WebLogDetailCode::InvalidParams),
        format_result_badge(status_code::BAD_REQUEST, None)
    );
    (category, color, detail)
}

fn render_command_result(
    client_ip: &str,
    role_key: &str,
    command: &str,
    added_bone_dagger: bool,
    status: u16,
    err_detail: Option<&str>,
) -> (LogCategoryCode, &'static str, String) {
    let role_label: String = sanitize_role_label(role_key);
    let cmd_preview: String = format_console_command_preview(command);
    let extra_note: &str = if added_bone_dagger {
        languages::web_detail_text(WebLogDetailCode::BoneDaggerAddedSuffix)
    } else {
        ""
    };
    let detail: String = format!(
        "{client_ip} | {role_label} -> {cmd_preview}{extra_note} {}",
        format_result_badge(status, err_detail)
    );
    (LogCategoryCode::Command, colors::LIGHT_CYAN, detail)
}

fn render_status_invalid(
    client_ip: &str,
    role_key: &str,
    prop_name: &str,
    raw_value: &str,
) -> (LogCategoryCode, &'static str, String) {
    let role_label: String = sanitize_role_label(role_key);
    let safe_prop: String = sanitize_log_text(prop_name, 32);
    let safe_val: String = sanitize_log_text(raw_value, 16);
    let reason: &str = languages::web_detail_text(WebLogDetailCode::InvalidNumberReason);
    let detail: String = format!(
        "{client_ip} | {role_label} | {safe_prop} = {safe_val} {}",
        format_result_badge(status_code::INVALID_VALUE, Some(reason))
    );
    (LogCategoryCode::Status, colors::LIGHT_YELLOW, detail)
}

fn render_status_result(
    client_ip: &str,
    role_key: &str,
    prop_name: &str,
    value_text: &str,
    status: u16,
    err_detail: Option<&str>,
) -> (LogCategoryCode, &'static str, String) {
    let role_label: String = sanitize_role_label(role_key);
    let safe_prop: String = sanitize_log_text(prop_name, 32);
    let detail: String = format!(
        "{client_ip} | {role_label} | {safe_prop} = {value_text} {}",
        format_result_badge(status, err_detail)
    );
    (LogCategoryCode::Status, colors::LIGHT_BLUE, detail)
}

fn render_item_invalid(
    client_ip: &str,
    role_key: &str,
    item_key: &str,
    raw_count: &str,
) -> (LogCategoryCode, &'static str, String) {
    let role_label: String = sanitize_role_label(role_key);
    let item_label: String = sanitize_item_label(item_key);
    let safe_count: String = sanitize_log_text(raw_count, 12);
    let reason: &str = languages::web_detail_text(WebLogDetailCode::InvalidCountReason);
    let detail: String = format!(
        "{client_ip} | {role_label} | {item_label} x{safe_count} {}",
        format_result_badge(status_code::INVALID_VALUE, Some(reason))
    );
    (LogCategoryCode::Item, colors::LIGHT_YELLOW, detail)
}

fn render_item_result(
    client_ip: &str,
    role_key: &str,
    item_key: &str,
    count: i64,
    status: u16,
    err_detail: Option<&str>,
) -> (LogCategoryCode, &'static str, String) {
    let role_label: String = sanitize_role_label(role_key);
    let item_label: String = sanitize_item_label(item_key);
    let detail: String = format!(
        "{client_ip} | {role_label} | {item_label} x{count} {}",
        format_result_badge(status, err_detail)
    );
    (LogCategoryCode::Item, colors::LIGHT_GREEN, detail)
}

fn render_storage_invalid(
    client_ip: &str,
    role_key: &str,
    raw_limit: &str,
) -> (LogCategoryCode, &'static str, String) {
    let role_label: String = sanitize_role_label(role_key);
    let cap_label: &str = languages::web_detail_text(WebLogDetailCode::StorageCapacityLabel);
    let safe_limit: String = sanitize_log_text(raw_limit, 12);
    let reason: &str = languages::web_detail_text(WebLogDetailCode::InvalidValueReason);
    let detail: String = format!(
        "{client_ip} | {role_label} | {cap_label} {safe_limit} {}",
        format_result_badge(status_code::INVALID_VALUE, Some(reason))
    );
    (LogCategoryCode::Storage, colors::LIGHT_YELLOW, detail)
}

fn render_storage_result(
    client_ip: &str,
    role_key: &str,
    limit: i64,
    status: u16,
    err_detail: Option<&str>,
) -> (LogCategoryCode, &'static str, String) {
    let role_label: String = sanitize_role_label(role_key);
    let cap_label: &str = languages::web_detail_text(WebLogDetailCode::StorageCapacityLabel);
    let detail: String = format!(
        "{client_ip} | {role_label} | {cap_label} {limit} {}",
        format_result_badge(status, err_detail)
    );
    (LogCategoryCode::Storage, colors::LIGHT_MAGENTA, detail)
}

fn render_notice_result(
    client_ip: &str,
    role_key: &str,
    message: &str,
    status: u16,
    err_detail: Option<&str>,
) -> (LogCategoryCode, &'static str, String) {
    let role_label: String = sanitize_role_label(role_key);
    let safe_msg: String = sanitize_log_text(message, 48);
    let quoted: String = languages::quote_message(&safe_msg);
    let detail: String = format!(
        "{client_ip} | {role_label} -> {quoted} {}",
        format_result_badge(status, err_detail)
    );
    (LogCategoryCode::Notice, colors::LIGHT_YELLOW, detail)
}

fn render_broadcast_result(
    client_ip: &str,
    message: &str,
    status: u16,
    err_detail: Option<&str>,
) -> (LogCategoryCode, &'static str, String) {
    let target_all: &str = languages::web_detail_text(WebLogDetailCode::BroadcastAllTarget);
    let safe_msg: String = sanitize_log_text(message, 48);
    let quoted: String = languages::quote_message(&safe_msg);
    let detail: String = format!(
        "{client_ip} | {target_all} -> {quoted} {}",
        format_result_badge(status, err_detail)
    );
    (LogCategoryCode::Broadcast, colors::LIGHT_YELLOW, detail)
}

fn render_kick_result(
    client_ip: &str,
    role_key: &str,
    status: u16,
    err_detail: Option<&str>,
) -> (LogCategoryCode, &'static str, String) {
    let role_label: String = sanitize_role_label(role_key);
    let kick_verb: &str = languages::web_detail_text(WebLogDetailCode::KickVerb);
    let detail: String = format!(
        "{client_ip} | {role_label} {kick_verb} {}",
        format_result_badge(status, err_detail)
    );
    (LogCategoryCode::Kick, colors::LIGHT_RED, detail)
}

fn resolve_web_log_parts(event: &WebLogEvent<'_>) -> (LogCategoryCode, &'static str, String) {
    match event {
        WebLogEvent::AuthUnauthenticatedRejected {
            client_ip,
            pathname,
        } => (
            LogCategoryCode::Auth,
            colors::RED,
            languages::render_unauthenticated_rejected(client_ip, pathname),
        ),
        WebLogEvent::AuthInvalidRequestBody {
            client_ip,
            pathname,
        } => (
            LogCategoryCode::Error,
            colors::RED,
            languages::render_invalid_request_body(client_ip, pathname),
        ),
        WebLogEvent::AuthDecryptFailed {
            client_ip,
            pathname,
        } => (
            LogCategoryCode::Auth,
            colors::RED,
            languages::render_decrypt_failed(client_ip, pathname),
        ),
        WebLogEvent::AuthInvalidJsonParams {
            client_ip,
            pathname,
        } => (
            LogCategoryCode::Error,
            colors::LIGHT_YELLOW,
            languages::render_invalid_json_params(client_ip, pathname),
        ),
        WebLogEvent::AuthLoginFailedInvalidBody { client_ip } => (
            LogCategoryCode::Auth,
            colors::RED,
            languages::render_login_failed_invalid_body(client_ip),
        ),
        WebLogEvent::AuthLoginFailedPskMismatch { client_ip } => (
            LogCategoryCode::Auth,
            colors::RED,
            languages::render_login_failed_psk_mismatch(client_ip),
        ),
        WebLogEvent::AuthLoginFailedChallengeInvalid { client_ip } => (
            LogCategoryCode::Auth,
            colors::LIGHT_YELLOW,
            languages::render_login_failed_challenge_invalid(client_ip),
        ),
        WebLogEvent::AuthLoginSucceeded { client_ip } => (
            LogCategoryCode::Auth,
            colors::LIGHT_GREEN,
            languages::render_login_succeeded(client_ip),
        ),
        WebLogEvent::Startup {
            web_port,
            patch_port,
            auto_broadcast,
        } => (
            LogCategoryCode::Startup,
            colors::LIGHT_GREEN,
            languages::render_startup_detail(*web_port, *patch_port, *auto_broadcast),
        ),
        WebLogEvent::Shutdown => (
            LogCategoryCode::Shutdown,
            colors::LIGHT_YELLOW,
            languages::web_detail_text(WebLogDetailCode::ShutdownDetail).to_string(),
        ),
        WebLogEvent::UrlCopied { is_public, success } => (
            LogCategoryCode::Copy,
            if *success {
                colors::LIGHT_GREEN
            } else {
                colors::RED
            },
            languages::render_copy_log(*is_public, *success),
        ),
        WebLogEvent::InvalidParams {
            category,
            client_ip,
        } => render_invalid_params(*category, client_ip),
        WebLogEvent::CommandResult {
            client_ip,
            role_key,
            command,
            added_bone_dagger,
            status,
            err_detail,
        } => render_command_result(
            client_ip,
            role_key,
            command,
            *added_bone_dagger,
            *status,
            *err_detail,
        ),
        WebLogEvent::StatusInvalidValue {
            client_ip,
            role_key,
            prop_name,
            raw_value,
        } => render_status_invalid(client_ip, role_key, prop_name, raw_value),
        WebLogEvent::StatusResult {
            client_ip,
            role_key,
            prop_name,
            value_text,
            status,
            err_detail,
        } => render_status_result(
            client_ip,
            role_key,
            prop_name,
            value_text,
            *status,
            *err_detail,
        ),
        WebLogEvent::ItemInvalidCount {
            client_ip,
            role_key,
            item_key,
            raw_count,
        } => render_item_invalid(client_ip, role_key, item_key, raw_count),
        WebLogEvent::ItemResult {
            client_ip,
            role_key,
            item_key,
            count,
            status,
            err_detail,
        } => render_item_result(client_ip, role_key, item_key, *count, *status, *err_detail),
        WebLogEvent::StorageInvalidValue {
            client_ip,
            role_key,
            raw_limit,
        } => render_storage_invalid(client_ip, role_key, raw_limit),
        WebLogEvent::StorageResult {
            client_ip,
            role_key,
            limit,
            status,
            err_detail,
        } => render_storage_result(client_ip, role_key, *limit, *status, *err_detail),
        WebLogEvent::NoticeResult {
            client_ip,
            role_key,
            message,
            status,
            err_detail,
        } => render_notice_result(client_ip, role_key, message, *status, *err_detail),
        WebLogEvent::BroadcastResult {
            client_ip,
            message,
            status,
            err_detail,
        } => render_broadcast_result(client_ip, message, *status, *err_detail),
        WebLogEvent::KickResult {
            client_ip,
            role_key,
            status,
            err_detail,
        } => render_kick_result(client_ip, role_key, *status, *err_detail),
    }
}

/// Renders a structured `WebLogEvent` into an ANSI-colored Tab 1 log line by resolving
/// all user-facing text from `crate::languages` right before display.
#[must_use]
pub fn render_web_log_event(event: &WebLogEvent<'_>) -> String {
    let (category, category_color, detail): (LogCategoryCode, &'static str, String) =
        resolve_web_log_parts(event);
    let category_label: &str = languages::log_category_text(category);
    format!(
        " {}[{}] {}[{}] {}{}",
        colors::LIGHT_BLACK,
        format_local_time(),
        category_color,
        category_label,
        colors::LIGHT_WHITE,
        detail,
    )
}

pub fn emit_web_log_event(sink: &WebLogSink, event: &WebLogEvent<'_>) {
    let line: String = render_web_log_event(event);
    (sink)(line);
}
