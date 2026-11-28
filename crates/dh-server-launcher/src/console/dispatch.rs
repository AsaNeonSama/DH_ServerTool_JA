use std::iter::Peekable;
use std::str::Chars;
use std::sync::Arc;

use crate::console::bridge::build_console_broadcast_message;
pub use crate::consts::console::{status_code, HIGH_STAT_WARNING_THRESHOLD, MAX_LOG_TEXT_CHARS};
use crate::consts::console::{BONE_DAGGER_ITEM, MAX_SAFE_INT_F64};
use crate::format::{format_local_time, sanitize_log_text};
use crate::languages;
pub use crate::structs::console::{
    ConsoleActionPayload, ConsoleDispatchContext, ConsoleLogRecord, ConsoleLogSink,
};
use crate::structs::languages::{
    ApiCode, BroadcastEvent, ConsoleLogDetailCode, ConsoleLogEvent, LogCategoryCode,
    ResultBadgeCode,
};

#[must_use]
pub fn noop_console_log_sink() -> ConsoleLogSink {
    Arc::new(|_: ConsoleLogRecord| {})
}

#[must_use]
fn action_result(status: u16, code: ApiCode) -> ConsoleActionPayload {
    ConsoleActionPayload { status, code }
}

fn format_js_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() <= MAX_SAFE_INT_F64 {
        let int_text: String = format!("{value:.0}");
        if let Ok(int_val) = int_text.parse::<i64>() {
            return int_val.to_string();
        }
    }
    value.to_string()
}

/// Mimics `JavaScript` `Number.parseInt(text, 10)`.
fn parse_js_int_base10(text: &str) -> Option<i64> {
    let trimmed: &str = text.trim_start();
    let mut chars: Peekable<Chars<'_>> = trimmed.chars().peekable();
    let negative: bool = match chars.peek() {
        Some('-') => {
            let _: Option<char> = chars.next();
            true
        }
        Some('+') => {
            let _: Option<char> = chars.next();
            false
        }
        _ => false,
    };

    let mut saw_digit: bool = false;
    let mut digits: String = String::with_capacity(20);
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            let _: Option<char> = chars.next();
            if digits.is_empty() && ch == '0' {
                continue;
            }
            if digits.len() >= 20 {
                return None;
            }
            digits.push(ch);
        } else {
            break;
        }
    }
    if !saw_digit {
        return None;
    }
    if digits.is_empty() {
        return Some(0);
    }
    let mut signed: String = String::with_capacity(digits.len() + usize::from(negative));
    if negative {
        signed.push('-');
    }
    signed.push_str(&digits);
    signed.parse::<i64>().ok()
}

#[must_use]
pub fn format_result_badge(status: u16, err_detail: Option<&str>) -> String {
    if status == status_code::SUCCESS {
        return languages::render_result_badge(ResultBadgeCode::Sent, "");
    }
    if status == status_code::PARTIAL_SUCCESS {
        return languages::render_result_badge(ResultBadgeCode::PartialFailure, "");
    }
    let suffix: String = match err_detail {
        Some(detail) if !detail.is_empty() => format!(": {}", sanitize_log_text(detail, 36)),
        _ => String::new(),
    };
    if status == status_code::SERVER_ERROR {
        languages::render_result_badge(ResultBadgeCode::SendFailed, &suffix)
    } else {
        languages::render_result_badge(ResultBadgeCode::Invalid, &suffix)
    }
}

#[must_use]
pub fn format_console_command_preview(command: &str) -> String {
    let trimmed: &str = command.trim();
    let (raw_cmd_name, cmd_arg): (&str, &str) = match trimmed.split_once(' ') {
        Some((name, rest)) => (name, rest.trim()),
        None => (trimmed, ""),
    };
    let cmd_name: String = raw_cmd_name.to_ascii_lowercase();
    if cmd_name == "summon" && !cmd_arg.is_empty() {
        if let Some(known_name) = languages::actor_display_name(cmd_arg) {
            return format!("summon {known_name}");
        }
    }
    if cmd_name == "endmatch" {
        let side_code: ConsoleLogDetailCode =
            if cmd_arg == "0" || cmd_arg.eq_ignore_ascii_case("false") {
                ConsoleLogDetailCode::EndMatchThrallsWin
            } else {
                ConsoleLogDetailCode::EndMatchExplorersWin
            };
        return format!("endmatch {}", languages::console_detail_text(side_code));
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

fn tone_for_status_and_category(category: LogCategoryCode, status: u16) -> &'static str {
    if status == status_code::SERVER_ERROR {
        return "error";
    }
    if status != status_code::SUCCESS {
        return "warn";
    }
    match category {
        LogCategoryCode::Item => "success",
        LogCategoryCode::Status => "blue",
        LogCategoryCode::Storage => "purple",
        LogCategoryCode::Notice | LogCategoryCode::Broadcast => "warn",
        LogCategoryCode::Kick => "error",
        LogCategoryCode::Command => "info",
    }
}

/// Renders a structured `ConsoleLogEvent` into an ANSI-free `ConsoleLogRecord`.
#[must_use]
pub fn render_console_log_record(event: &ConsoleLogEvent<'_>) -> ConsoleLogRecord {
    let (category, tone, detail): (LogCategoryCode, &'static str, String) = match event {
        ConsoleLogEvent::InvalidParams { category } => (
            *category,
            "warn",
            format!(
                "{} {}",
                languages::console_detail_text(ConsoleLogDetailCode::InvalidParams),
                format_result_badge(status_code::BAD_REQUEST, None)
            ),
        ),
        ConsoleLogEvent::CommandResult {
            role_key,
            command,
            added_bone_dagger,
            status,
            err_detail,
        } => {
            let role_label: String = sanitize_role_label(role_key);
            let cmd_preview: String = format_console_command_preview(command);
            let extra_note: &str = if *added_bone_dagger {
                languages::console_detail_text(ConsoleLogDetailCode::BoneDaggerAddedSuffix)
            } else {
                ""
            };
            (
                LogCategoryCode::Command,
                tone_for_status_and_category(LogCategoryCode::Command, *status),
                format!(
                    "{role_label} -> {cmd_preview}{extra_note} {}",
                    format_result_badge(*status, *err_detail)
                ),
            )
        }
        ConsoleLogEvent::StatusInvalidValue {
            role_key,
            prop_name,
            raw_value,
        } => {
            let role_label: String = sanitize_role_label(role_key);
            let safe_prop: String = sanitize_log_text(prop_name, 32);
            let safe_val: String = sanitize_log_text(raw_value, 16);
            let reason: &str =
                languages::console_detail_text(ConsoleLogDetailCode::InvalidNumberReason);
            (
                LogCategoryCode::Status,
                "warn",
                format!(
                    "{role_label} | {safe_prop} = {safe_val} {}",
                    format_result_badge(status_code::INVALID_VALUE, Some(reason))
                ),
            )
        }
        ConsoleLogEvent::StatusResult {
            role_key,
            prop_name,
            value_text,
            status,
            err_detail,
        } => {
            let role_label: String = sanitize_role_label(role_key);
            let safe_prop: String = sanitize_log_text(prop_name, 32);
            (
                LogCategoryCode::Status,
                tone_for_status_and_category(LogCategoryCode::Status, *status),
                format!(
                    "{role_label} | {safe_prop} = {value_text} {}",
                    format_result_badge(*status, *err_detail)
                ),
            )
        }
        ConsoleLogEvent::ItemInvalidCount {
            role_key,
            item_key,
            raw_count,
        } => {
            let role_label: String = sanitize_role_label(role_key);
            let item_label: String = sanitize_item_label(item_key);
            let safe_count: String = sanitize_log_text(raw_count, 12);
            let reason: &str =
                languages::console_detail_text(ConsoleLogDetailCode::InvalidCountReason);
            (
                LogCategoryCode::Item,
                "warn",
                format!(
                    "{role_label} | {item_label} x{safe_count} {}",
                    format_result_badge(status_code::INVALID_VALUE, Some(reason))
                ),
            )
        }
        ConsoleLogEvent::ItemResult {
            role_key,
            item_key,
            count,
            status,
            err_detail,
        } => {
            let role_label: String = sanitize_role_label(role_key);
            let item_label: String = sanitize_item_label(item_key);
            (
                LogCategoryCode::Item,
                tone_for_status_and_category(LogCategoryCode::Item, *status),
                format!(
                    "{role_label} | {item_label} x{count} {}",
                    format_result_badge(*status, *err_detail)
                ),
            )
        }
        ConsoleLogEvent::StorageInvalidValue {
            role_key,
            raw_limit,
        } => {
            let role_label: String = sanitize_role_label(role_key);
            let cap_label: &str =
                languages::console_detail_text(ConsoleLogDetailCode::StorageCapacityLabel);
            let safe_limit: String = sanitize_log_text(raw_limit, 12);
            let reason: &str =
                languages::console_detail_text(ConsoleLogDetailCode::InvalidValueReason);
            (
                LogCategoryCode::Storage,
                "warn",
                format!(
                    "{role_label} | {cap_label} {safe_limit} {}",
                    format_result_badge(status_code::INVALID_VALUE, Some(reason))
                ),
            )
        }
        ConsoleLogEvent::StorageResult {
            role_key,
            limit,
            status,
            err_detail,
        } => {
            let role_label: String = sanitize_role_label(role_key);
            let cap_label: &str =
                languages::console_detail_text(ConsoleLogDetailCode::StorageCapacityLabel);
            (
                LogCategoryCode::Storage,
                tone_for_status_and_category(LogCategoryCode::Storage, *status),
                format!(
                    "{role_label} | {cap_label} {limit} {}",
                    format_result_badge(*status, *err_detail)
                ),
            )
        }
        ConsoleLogEvent::NoticeResult {
            role_key,
            message,
            status,
            err_detail,
        } => {
            let role_label: String = sanitize_role_label(role_key);
            let safe_msg: String = sanitize_log_text(message, 48);
            let quoted: String = languages::quote_message(&safe_msg);
            (
                LogCategoryCode::Notice,
                tone_for_status_and_category(LogCategoryCode::Notice, *status),
                format!(
                    "{role_label} -> {quoted} {}",
                    format_result_badge(*status, *err_detail)
                ),
            )
        }
        ConsoleLogEvent::BroadcastResult {
            message,
            status,
            err_detail,
        } => {
            let target_all: &str =
                languages::console_detail_text(ConsoleLogDetailCode::BroadcastAllTarget);
            let safe_msg: String = sanitize_log_text(message, 48);
            let quoted: String = languages::quote_message(&safe_msg);
            (
                LogCategoryCode::Broadcast,
                tone_for_status_and_category(LogCategoryCode::Broadcast, *status),
                format!(
                    "{target_all} -> {quoted} {}",
                    format_result_badge(*status, *err_detail)
                ),
            )
        }
        ConsoleLogEvent::KickResult {
            role_key,
            status,
            err_detail,
        } => {
            let role_label: String = sanitize_role_label(role_key);
            let kick_verb: &str = languages::console_detail_text(ConsoleLogDetailCode::KickVerb);
            (
                LogCategoryCode::Kick,
                tone_for_status_and_category(LogCategoryCode::Kick, *status),
                format!(
                    "{role_label} {kick_verb} {}",
                    format_result_badge(*status, *err_detail)
                ),
            )
        }
    };

    ConsoleLogRecord {
        time: format_local_time(),
        category: languages::log_category_text(category).to_string(),
        tone: tone.to_string(),
        detail,
    }
}

pub fn emit_console_log_event(sink: &ConsoleLogSink, event: &ConsoleLogEvent<'_>) {
    let record: ConsoleLogRecord = render_console_log_record(event);
    (sink)(record);
}

pub async fn handle_console_command(
    ctx: &ConsoleDispatchContext<'_>,
    player_name: &str,
    command: &str,
) -> ConsoleActionPayload {
    let target_role: &str = player_name.trim();
    let trimmed_command: &str = command.trim();
    if target_role.is_empty() || trimmed_command.is_empty() {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::InvalidParams {
                category: LogCategoryCode::Command,
            },
        );
        return action_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    }

    let (raw_command_name, command_arg): (&str, &str) = match trimmed_command.split_once(' ') {
        Some((name, rest)) => (name, rest.trim()),
        None => (trimmed_command, ""),
    };
    let command_name: String = raw_command_name.to_ascii_lowercase();
    let normalized_command: String = if command_arg.is_empty() {
        command_name.clone()
    } else {
        format!("{command_name} {command_arg}")
    };

    if let Err(err) = ctx
        .bridge
        .execute_console_command(target_role, &normalized_command)
        .await
    {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::CommandResult {
                role_key: target_role,
                command: &normalized_command,
                added_bone_dagger: false,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return action_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
    }

    let mut added_bone_dagger: bool = false;
    if command_name == "setisthrall" && command_arg != "0" {
        added_bone_dagger = true;
        if ctx
            .bridge
            .add_inventory_item(target_role, BONE_DAGGER_ITEM, 1)
            .await
            .is_err()
        {
            emit_console_log_event(
                ctx.log_sink,
                &ConsoleLogEvent::CommandResult {
                    role_key: target_role,
                    command: &normalized_command,
                    added_bone_dagger,
                    status: status_code::PARTIAL_SUCCESS,
                    err_detail: None,
                },
            );
            return action_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
        }
    }

    if ctx.auto_broadcast {
        if let Some(notice) =
            build_console_broadcast_message(target_role, &command_name, command_arg)
        {
            if ctx
                .bridge
                .broadcast_message_except(target_role, &notice)
                .await
                .is_err()
            {
                emit_console_log_event(
                    ctx.log_sink,
                    &ConsoleLogEvent::CommandResult {
                        role_key: target_role,
                        command: &normalized_command,
                        added_bone_dagger,
                        status: status_code::PARTIAL_SUCCESS,
                        err_detail: None,
                    },
                );
                return action_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
            }
        }
    }

    emit_console_log_event(
        ctx.log_sink,
        &ConsoleLogEvent::CommandResult {
            role_key: target_role,
            command: &normalized_command,
            added_bone_dagger,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    action_result(status_code::SUCCESS, ApiCode::Success)
}

pub async fn handle_set_human(
    ctx: &ConsoleDispatchContext<'_>,
    player_name: &str,
    prop_name: &str,
    raw_value: &str,
) -> ConsoleActionPayload {
    let target_role: &str = player_name.trim();
    let property_name: &str = prop_name.trim();
    let trimmed_value: &str = raw_value.trim();

    if target_role.is_empty() || property_name.is_empty() || trimmed_value.is_empty() {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::InvalidParams {
                category: LogCategoryCode::Status,
            },
        );
        return action_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    }

    let Some(property_value): Option<f64> = trimmed_value
        .parse::<f64>()
        .ok()
        .filter(|v: &f64| v.is_finite() && *v >= 0.0)
    else {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::StatusInvalidValue {
                role_key: target_role,
                prop_name: property_name,
                raw_value: trimmed_value,
            },
        );
        return action_result(status_code::INVALID_VALUE, ApiCode::InvalidValueError);
    };

    let value_str: String = format_js_number(property_value);
    if let Err(err) = ctx
        .bridge
        .set_human_property(target_role, property_name, property_value)
        .await
    {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::StatusResult {
                role_key: target_role,
                prop_name: property_name,
                value_text: &value_str,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return action_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
    }

    if ctx.auto_broadcast {
        let notice: String = languages::render_broadcast(&BroadcastEvent::ModifiedStats {
            role_key: target_role,
        });
        if ctx
            .bridge
            .broadcast_message_except(target_role, &notice)
            .await
            .is_err()
        {
            emit_console_log_event(
                ctx.log_sink,
                &ConsoleLogEvent::StatusResult {
                    role_key: target_role,
                    prop_name: property_name,
                    value_text: &value_str,
                    status: status_code::PARTIAL_SUCCESS,
                    err_detail: None,
                },
            );
            return action_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
        }
    }

    emit_console_log_event(
        ctx.log_sink,
        &ConsoleLogEvent::StatusResult {
            role_key: target_role,
            prop_name: property_name,
            value_text: &value_str,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    if property_value > HIGH_STAT_WARNING_THRESHOLD {
        return action_result(status_code::SUCCESS, ApiCode::HighValueWarning);
    }
    action_result(status_code::SUCCESS, ApiCode::Success)
}

pub async fn handle_add_item(
    ctx: &ConsoleDispatchContext<'_>,
    player_name: &str,
    item_name: &str,
    raw_count: &str,
) -> ConsoleActionPayload {
    let target_role: &str = player_name.trim();
    let item_blueprint_path: &str = item_name.trim();
    let count_str: &str = raw_count.trim();

    if target_role.is_empty() || item_blueprint_path.is_empty() || count_str.is_empty() {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::InvalidParams {
                category: LogCategoryCode::Item,
            },
        );
        return action_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    }

    let Some(item_count): Option<i64> = parse_js_int_base10(count_str).filter(|c: &i64| *c > 0)
    else {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::ItemInvalidCount {
                role_key: target_role,
                item_key: item_blueprint_path,
                raw_count: count_str,
            },
        );
        return action_result(status_code::INVALID_VALUE, ApiCode::InvalidValueError);
    };

    if let Err(err) = ctx
        .bridge
        .add_inventory_item(target_role, item_blueprint_path, item_count)
        .await
    {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::ItemResult {
                role_key: target_role,
                item_key: item_blueprint_path,
                count: item_count,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return action_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
    }

    if ctx.auto_broadcast {
        let notice: String = languages::render_broadcast(&BroadcastEvent::AcquiredItem {
            role_key: target_role,
            item_key: item_blueprint_path,
            count: item_count,
        });
        if ctx
            .bridge
            .broadcast_message_except(target_role, &notice)
            .await
            .is_err()
        {
            emit_console_log_event(
                ctx.log_sink,
                &ConsoleLogEvent::ItemResult {
                    role_key: target_role,
                    item_key: item_blueprint_path,
                    count: item_count,
                    status: status_code::PARTIAL_SUCCESS,
                    err_detail: None,
                },
            );
            return action_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
        }
    }

    emit_console_log_event(
        ctx.log_sink,
        &ConsoleLogEvent::ItemResult {
            role_key: target_role,
            item_key: item_blueprint_path,
            count: item_count,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    action_result(status_code::SUCCESS, ApiCode::Success)
}

pub async fn handle_set_storage(
    ctx: &ConsoleDispatchContext<'_>,
    player_name: &str,
    raw_limit: &str,
) -> ConsoleActionPayload {
    let target_role: &str = player_name.trim();
    let limit_str: &str = raw_limit.trim();

    if target_role.is_empty() || limit_str.is_empty() {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::InvalidParams {
                category: LogCategoryCode::Storage,
            },
        );
        return action_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    }

    let Some(storage_limit): Option<i64> = parse_js_int_base10(limit_str).filter(|l: &i64| *l > 0)
    else {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::StorageInvalidValue {
                role_key: target_role,
                raw_limit: limit_str,
            },
        );
        return action_result(status_code::INVALID_VALUE, ApiCode::InvalidValueError);
    };

    if let Err(err) = ctx
        .bridge
        .set_player_storage_limit(target_role, storage_limit)
        .await
    {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::StorageResult {
                role_key: target_role,
                limit: storage_limit,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return action_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
    }

    if ctx.auto_broadcast {
        let notice: String = languages::render_broadcast(&BroadcastEvent::StorageLimitChanged {
            role_key: target_role,
            limit: storage_limit,
        });
        if ctx
            .bridge
            .broadcast_message_except(target_role, &notice)
            .await
            .is_err()
        {
            emit_console_log_event(
                ctx.log_sink,
                &ConsoleLogEvent::StorageResult {
                    role_key: target_role,
                    limit: storage_limit,
                    status: status_code::PARTIAL_SUCCESS,
                    err_detail: None,
                },
            );
            return action_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
        }
    }

    emit_console_log_event(
        ctx.log_sink,
        &ConsoleLogEvent::StorageResult {
            role_key: target_role,
            limit: storage_limit,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    action_result(status_code::SUCCESS, ApiCode::Success)
}

fn normalize_chat_message(raw_message: &str) -> String {
    let without_nul: String = raw_message.replace('\0', "");
    let normalized_crlf: String = without_nul.replace("\r\n", "\n").replace('\r', "\n");
    normalized_crlf.trim().to_string()
}

pub async fn handle_send_thrall_msg(
    ctx: &ConsoleDispatchContext<'_>,
    player_name: &str,
    raw_message: &str,
) -> ConsoleActionPayload {
    let target_role: &str = player_name.trim();
    let normalized_msg: String = normalize_chat_message(raw_message);
    let message: &str = normalized_msg.as_str();

    if target_role.is_empty() || message.is_empty() {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::InvalidParams {
                category: LogCategoryCode::Notice,
            },
        );
        return action_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    }

    if let Err(err) = ctx.bridge.send_player_message(target_role, message).await {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::NoticeResult {
                role_key: target_role,
                message,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return action_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
    }

    emit_console_log_event(
        ctx.log_sink,
        &ConsoleLogEvent::NoticeResult {
            role_key: target_role,
            message,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    action_result(status_code::SUCCESS, ApiCode::Success)
}

pub async fn handle_broadcast(
    ctx: &ConsoleDispatchContext<'_>,
    raw_message: &str,
) -> ConsoleActionPayload {
    let normalized_msg: String = normalize_chat_message(raw_message);
    let message: &str = normalized_msg.as_str();
    if message.is_empty() {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::InvalidParams {
                category: LogCategoryCode::Broadcast,
            },
        );
        return action_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    }

    if let Err(err) = ctx.bridge.broadcast_message_to_all(message).await {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::BroadcastResult {
                message,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return action_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
    }

    emit_console_log_event(
        ctx.log_sink,
        &ConsoleLogEvent::BroadcastResult {
            message,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    action_result(status_code::SUCCESS, ApiCode::Success)
}

pub async fn handle_kick_player(
    ctx: &ConsoleDispatchContext<'_>,
    player_name: &str,
) -> ConsoleActionPayload {
    let target_role: &str = player_name.trim();
    if target_role.is_empty() {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::InvalidParams {
                category: LogCategoryCode::Kick,
            },
        );
        return action_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    }

    if let Err(err) = ctx.bridge.kick_target_player(target_role).await {
        emit_console_log_event(
            ctx.log_sink,
            &ConsoleLogEvent::KickResult {
                role_key: target_role,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return action_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
    }

    if ctx.auto_broadcast {
        let notice: String = languages::render_broadcast(&BroadcastEvent::Kicked {
            role_key: target_role,
        });
        if ctx
            .bridge
            .broadcast_message_except(target_role, &notice)
            .await
            .is_err()
        {
            emit_console_log_event(
                ctx.log_sink,
                &ConsoleLogEvent::KickResult {
                    role_key: target_role,
                    status: status_code::PARTIAL_SUCCESS,
                    err_detail: None,
                },
            );
            return action_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
        }
    }

    emit_console_log_event(
        ctx.log_sink,
        &ConsoleLogEvent::KickResult {
            role_key: target_role,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    action_result(status_code::SUCCESS, ApiCode::Success)
}
