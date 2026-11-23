use std::iter::Peekable;
use std::str::Chars;

use serde_json::{Map, Value};

use crate::colors;
use crate::format::sanitize_log_text;
use crate::messages::ja::web_console::{
    actor_display_name, api, broadcasts, log_categories, log_details,
};
use crate::web_console::auth::{api_result, ApiResultPayload};
use crate::web_console::bridge::{
    build_console_broadcast_message, role_display, ConsoleBridge, BONE_DAGGER_ITEM,
};
use crate::web_console::logger::{
    emit_web_log, format_console_command_preview, format_result_badge, WebLogSink,
};

pub const HIGH_STAT_WARNING_THRESHOLD: f64 = 500.0;
const MAX_SAFE_INT_F64: f64 = 9_007_199_254_740_991.0;

pub mod status_code {
    pub const SUCCESS: u16 = 200;
    pub const BAD_REQUEST: u16 = 400;
    pub const SERVER_ERROR: u16 = 504;
    pub const INVALID_VALUE: u16 = 416;
    pub const PARTIAL_SUCCESS: u16 = 207;
}

fn to_string_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

fn js_number_from_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let trimmed: &str = s.trim();
            if trimmed.is_empty() {
                Some(0.0)
            } else {
                trimmed.parse::<f64>().ok()
            }
        }
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
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

    let mut digits: String = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            let _: Option<char> = chars.next();
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    let mut signed: String = String::with_capacity(digits.len() + usize::from(negative));
    if negative {
        signed.push('-');
    }
    signed.push_str(&digits);
    signed.parse::<i64>().ok()
}

pub struct ApiRouteContext<'a> {
    pub bridge: &'a ConsoleBridge,
    pub auto_broadcast: bool,
    pub log_sink: &'a WebLogSink,
}

pub async fn handle_console_command(
    ctx: &ApiRouteContext<'_>,
    data: &Map<String, Value>,
    client_ip: &str,
) -> ApiResultPayload {
    let Some(target_role): Option<&str> = data.get("player_name").and_then(Value::as_str) else {
        emit_web_log(
            ctx.log_sink,
            log_categories::COMMAND,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {} {}",
                log_details::INVALID_PARAMS,
                format_result_badge(status_code::BAD_REQUEST, None)
            ),
        );
        return api_result(status_code::BAD_REQUEST, api::NETWORK_PARAM_ERROR);
    };
    let Some(command): Option<&str> = data.get("command").and_then(Value::as_str) else {
        emit_web_log(
            ctx.log_sink,
            log_categories::COMMAND,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {} {}",
                log_details::INVALID_PARAMS,
                format_result_badge(status_code::BAD_REQUEST, None)
            ),
        );
        return api_result(status_code::BAD_REQUEST, api::NETWORK_PARAM_ERROR);
    };

    let role_label: String = sanitize_log_text(role_display(target_role), 24);
    let cmd_preview: String = format_console_command_preview(command);

    if let Err(err) = ctx
        .bridge
        .execute_console_command(target_role, command)
        .await
    {
        emit_web_log(
            ctx.log_sink,
            log_categories::COMMAND,
            colors::LIGHT_CYAN,
            &format!(
                "{client_ip} | {role_label} -> {cmd_preview} {}",
                format_result_badge(status_code::SERVER_ERROR, Some(&err))
            ),
        );
        return api_result(status_code::SERVER_ERROR, api::SERVER_EXECUTION_ERROR);
    }

    let (command_name, command_arg): (&str, &str) = match command.find(' ') {
        Some(idx) => (&command[..idx], &command[idx + 1..]),
        None => (command, ""),
    };
    let mut extra_note: &str = "";

    if command_name.eq_ignore_ascii_case("setisthrall") && command_arg.trim() != "0" {
        extra_note = log_details::BONE_DAGGER_ADDED_SUFFIX;
        if ctx
            .bridge
            .add_inventory_item(target_role, BONE_DAGGER_ITEM, 1)
            .await
            .is_err()
        {
            emit_web_log(
                ctx.log_sink,
                log_categories::COMMAND,
                colors::LIGHT_CYAN,
                &format!(
                    "{client_ip} | {role_label} -> {cmd_preview}{extra_note} {}",
                    format_result_badge(status_code::PARTIAL_SUCCESS, None)
                ),
            );
            return api_result(status_code::PARTIAL_SUCCESS, api::BROADCAST_PARTIAL_ERROR);
        }
    }

    if ctx.auto_broadcast {
        if let Some(notice) =
            build_console_broadcast_message(target_role, command_name, command_arg)
        {
            if ctx
                .bridge
                .broadcast_message_except(target_role, &notice)
                .await
                .is_err()
            {
                emit_web_log(
                    ctx.log_sink,
                    log_categories::COMMAND,
                    colors::LIGHT_CYAN,
                    &format!(
                        "{client_ip} | {role_label} -> {cmd_preview}{extra_note} {}",
                        format_result_badge(status_code::PARTIAL_SUCCESS, None)
                    ),
                );
                return api_result(status_code::PARTIAL_SUCCESS, api::BROADCAST_PARTIAL_ERROR);
            }
        }
    }

    emit_web_log(
        ctx.log_sink,
        log_categories::COMMAND,
        colors::LIGHT_CYAN,
        &format!(
            "{client_ip} | {role_label} -> {cmd_preview}{extra_note} {}",
            format_result_badge(status_code::SUCCESS, None)
        ),
    );
    api_result(status_code::SUCCESS, api::SUCCESS)
}

pub async fn handle_set_human(
    ctx: &ApiRouteContext<'_>,
    data: &Map<String, Value>,
    client_ip: &str,
) -> ApiResultPayload {
    let target_role_opt: Option<&str> = data.get("player_name").and_then(Value::as_str);
    let property_name_opt: Option<&str> = data.get("prop_name").and_then(Value::as_str);
    let raw_value_opt: Option<&Value> = data.get("value");

    let is_empty_string: bool = raw_value_opt
        .and_then(Value::as_str)
        .is_some_and(str::is_empty);
    let non_null_value_opt: Option<&Value> = raw_value_opt.filter(|v: &&Value| !v.is_null());
    let (Some(target_role), Some(property_name), Some(raw_value)): (
        Option<&str>,
        Option<&str>,
        Option<&Value>,
    ) = (target_role_opt, property_name_opt, non_null_value_opt) else {
        emit_web_log(
            ctx.log_sink,
            log_categories::STATUS,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {} {}",
                log_details::INVALID_PARAMS,
                format_result_badge(status_code::BAD_REQUEST, None)
            ),
        );
        return api_result(status_code::BAD_REQUEST, api::NETWORK_PARAM_ERROR);
    };
    if is_empty_string {
        emit_web_log(
            ctx.log_sink,
            log_categories::STATUS,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {} {}",
                log_details::INVALID_PARAMS,
                format_result_badge(status_code::BAD_REQUEST, None)
            ),
        );
        return api_result(status_code::BAD_REQUEST, api::NETWORK_PARAM_ERROR);
    }

    let role_label: String = sanitize_log_text(role_display(target_role), 24);
    let safe_prop: String = sanitize_log_text(property_name, 32);
    let parsed_value_opt: Option<f64> = js_number_from_value(raw_value);
    let Some(property_value): Option<f64> =
        parsed_value_opt.filter(|v: &f64| v.is_finite() && *v >= 0.0)
    else {
        emit_web_log(
            ctx.log_sink,
            log_categories::STATUS,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {role_label} | {safe_prop} = {} {}",
                sanitize_log_text(&to_string_value(raw_value), 16),
                format_result_badge(
                    status_code::INVALID_VALUE,
                    Some(log_details::INVALID_NUMBER_REASON)
                )
            ),
        );
        return api_result(status_code::INVALID_VALUE, api::INVALID_VALUE_ERROR);
    };

    let value_str: String = format_js_number(property_value);
    if let Err(err) = ctx
        .bridge
        .set_human_property(target_role, property_name, property_value)
        .await
    {
        emit_web_log(
            ctx.log_sink,
            log_categories::STATUS,
            colors::LIGHT_BLUE,
            &format!(
                "{client_ip} | {role_label} | {safe_prop} = {value_str} {}",
                format_result_badge(status_code::SERVER_ERROR, Some(&err))
            ),
        );
        return api_result(status_code::SERVER_ERROR, api::SERVER_EXECUTION_ERROR);
    }

    if ctx.auto_broadcast {
        let notice: String = broadcasts::modified_stats(role_display(target_role));
        if ctx
            .bridge
            .broadcast_message_except(target_role, &notice)
            .await
            .is_err()
        {
            emit_web_log(
                ctx.log_sink,
                log_categories::STATUS,
                colors::LIGHT_BLUE,
                &format!(
                    "{client_ip} | {role_label} | {safe_prop} = {value_str} {}",
                    format_result_badge(status_code::PARTIAL_SUCCESS, None)
                ),
            );
            return api_result(status_code::PARTIAL_SUCCESS, api::BROADCAST_PARTIAL_ERROR);
        }
    }

    emit_web_log(
        ctx.log_sink,
        log_categories::STATUS,
        colors::LIGHT_BLUE,
        &format!(
            "{client_ip} | {role_label} | {safe_prop} = {value_str} {}",
            format_result_badge(status_code::SUCCESS, None)
        ),
    );
    if property_value > HIGH_STAT_WARNING_THRESHOLD {
        return api_result(status_code::SUCCESS, api::HIGH_VALUE_WARNING);
    }
    api_result(status_code::SUCCESS, api::SUCCESS)
}

pub async fn handle_add_item(
    ctx: &ApiRouteContext<'_>,
    data: &Map<String, Value>,
    client_ip: &str,
) -> ApiResultPayload {
    let target_role_opt: Option<&str> = data.get("player_name").and_then(Value::as_str);
    let item_blueprint_path_opt: Option<&str> = data.get("item_name").and_then(Value::as_str);
    let raw_item_count_opt: Option<&Value> = data.get("count").filter(|v: &&Value| !v.is_null());

    let (Some(target_role), Some(item_blueprint_path), Some(raw_item_count)): (
        Option<&str>,
        Option<&str>,
        Option<&Value>,
    ) = (target_role_opt, item_blueprint_path_opt, raw_item_count_opt) else {
        emit_web_log(
            ctx.log_sink,
            log_categories::ITEM,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {} {}",
                log_details::INVALID_PARAMS,
                format_result_badge(status_code::BAD_REQUEST, None)
            ),
        );
        return api_result(status_code::BAD_REQUEST, api::NETWORK_PARAM_ERROR);
    };

    let role_label: String = sanitize_log_text(role_display(target_role), 24);
    let item_label: String = sanitize_log_text(
        actor_display_name(item_blueprint_path).unwrap_or(item_blueprint_path),
        40,
    );
    let count_str: String = to_string_value(raw_item_count);
    let Some(item_count): Option<i64> = parse_js_int_base10(&count_str).filter(|c: &i64| *c > 0)
    else {
        emit_web_log(
            ctx.log_sink,
            log_categories::ITEM,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {role_label} | {item_label} x{} {}",
                sanitize_log_text(&count_str, 12),
                format_result_badge(
                    status_code::INVALID_VALUE,
                    Some(log_details::INVALID_COUNT_REASON)
                )
            ),
        );
        return api_result(status_code::INVALID_VALUE, api::INVALID_VALUE_ERROR);
    };

    if let Err(err) = ctx
        .bridge
        .add_inventory_item(target_role, item_blueprint_path, item_count)
        .await
    {
        emit_web_log(
            ctx.log_sink,
            log_categories::ITEM,
            colors::LIGHT_GREEN,
            &format!(
                "{client_ip} | {role_label} | {item_label} x{item_count} {}",
                format_result_badge(status_code::SERVER_ERROR, Some(&err))
            ),
        );
        return api_result(status_code::SERVER_ERROR, api::SERVER_EXECUTION_ERROR);
    }

    if ctx.auto_broadcast {
        let broadcast_item_label: &str =
            actor_display_name(item_blueprint_path).unwrap_or(item_blueprint_path);
        let notice: String =
            broadcasts::acquired_item(role_display(target_role), broadcast_item_label, item_count);
        if ctx
            .bridge
            .broadcast_message_except(target_role, &notice)
            .await
            .is_err()
        {
            emit_web_log(
                ctx.log_sink,
                log_categories::ITEM,
                colors::LIGHT_GREEN,
                &format!(
                    "{client_ip} | {role_label} | {item_label} x{item_count} {}",
                    format_result_badge(status_code::PARTIAL_SUCCESS, None)
                ),
            );
            return api_result(status_code::PARTIAL_SUCCESS, api::BROADCAST_PARTIAL_ERROR);
        }
    }

    emit_web_log(
        ctx.log_sink,
        log_categories::ITEM,
        colors::LIGHT_GREEN,
        &format!(
            "{client_ip} | {role_label} | {item_label} x{item_count} {}",
            format_result_badge(status_code::SUCCESS, None)
        ),
    );
    api_result(status_code::SUCCESS, api::SUCCESS)
}

pub async fn handle_set_storage(
    ctx: &ApiRouteContext<'_>,
    data: &Map<String, Value>,
    client_ip: &str,
) -> ApiResultPayload {
    let target_role_opt: Option<&str> = data.get("player_name").and_then(Value::as_str);
    let raw_limit_opt: Option<&Value> = data.get("limit").filter(|v: &&Value| !v.is_null());

    let (Some(target_role), Some(raw_limit)): (Option<&str>, Option<&Value>) =
        (target_role_opt, raw_limit_opt)
    else {
        emit_web_log(
            ctx.log_sink,
            log_categories::STORAGE,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {} {}",
                log_details::INVALID_PARAMS,
                format_result_badge(status_code::BAD_REQUEST, None)
            ),
        );
        return api_result(status_code::BAD_REQUEST, api::NETWORK_PARAM_ERROR);
    };

    let role_label: String = sanitize_log_text(role_display(target_role), 24);
    let limit_str: String = to_string_value(raw_limit);
    let Some(storage_limit): Option<i64> = parse_js_int_base10(&limit_str).filter(|l: &i64| *l > 0)
    else {
        emit_web_log(
            ctx.log_sink,
            log_categories::STORAGE,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {role_label} | {} {} {}",
                log_details::STORAGE_CAPACITY_LABEL,
                sanitize_log_text(&limit_str, 12),
                format_result_badge(
                    status_code::INVALID_VALUE,
                    Some(log_details::INVALID_VALUE_REASON)
                )
            ),
        );
        return api_result(status_code::INVALID_VALUE, api::INVALID_VALUE_ERROR);
    };

    if let Err(err) = ctx
        .bridge
        .set_player_storage_limit(target_role, storage_limit)
        .await
    {
        emit_web_log(
            ctx.log_sink,
            log_categories::STORAGE,
            colors::LIGHT_MAGENTA,
            &format!(
                "{client_ip} | {role_label} | {} {storage_limit} {}",
                log_details::STORAGE_CAPACITY_LABEL,
                format_result_badge(status_code::SERVER_ERROR, Some(&err))
            ),
        );
        return api_result(status_code::SERVER_ERROR, api::SERVER_EXECUTION_ERROR);
    }

    if ctx.auto_broadcast {
        let notice: String =
            broadcasts::storage_limit_changed(role_display(target_role), storage_limit);
        if ctx
            .bridge
            .broadcast_message_except(target_role, &notice)
            .await
            .is_err()
        {
            emit_web_log(
                ctx.log_sink,
                log_categories::STORAGE,
                colors::LIGHT_MAGENTA,
                &format!(
                    "{client_ip} | {role_label} | {} {storage_limit} {}",
                    log_details::STORAGE_CAPACITY_LABEL,
                    format_result_badge(status_code::PARTIAL_SUCCESS, None)
                ),
            );
            return api_result(status_code::PARTIAL_SUCCESS, api::BROADCAST_PARTIAL_ERROR);
        }
    }

    emit_web_log(
        ctx.log_sink,
        log_categories::STORAGE,
        colors::LIGHT_MAGENTA,
        &format!(
            "{client_ip} | {role_label} | {} {storage_limit} {}",
            log_details::STORAGE_CAPACITY_LABEL,
            format_result_badge(status_code::SUCCESS, None)
        ),
    );
    api_result(status_code::SUCCESS, api::SUCCESS)
}

pub async fn handle_send_thrall_msg(
    ctx: &ApiRouteContext<'_>,
    data: &Map<String, Value>,
    client_ip: &str,
) -> ApiResultPayload {
    let target_role_opt: Option<&str> = data.get("player_name").and_then(Value::as_str);
    let message_opt: Option<&str> = data.get("msg").and_then(Value::as_str);

    let (Some(target_role), Some(message)): (Option<&str>, Option<&str>) =
        (target_role_opt, message_opt)
    else {
        emit_web_log(
            ctx.log_sink,
            log_categories::NOTICE,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {} {}",
                log_details::INVALID_PARAMS,
                format_result_badge(status_code::BAD_REQUEST, None)
            ),
        );
        return api_result(status_code::BAD_REQUEST, api::NETWORK_PARAM_ERROR);
    };

    let role_label: String = sanitize_log_text(role_display(target_role), 24);
    let safe_msg: String = sanitize_log_text(message, 48);

    if let Err(err) = ctx.bridge.send_player_message(target_role, message).await {
        emit_web_log(
            ctx.log_sink,
            log_categories::NOTICE,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {role_label} -> 「{safe_msg}」 {}",
                format_result_badge(status_code::SERVER_ERROR, Some(&err))
            ),
        );
        return api_result(status_code::SERVER_ERROR, api::SERVER_EXECUTION_ERROR);
    }

    emit_web_log(
        ctx.log_sink,
        log_categories::NOTICE,
        colors::LIGHT_YELLOW,
        &format!(
            "{client_ip} | {role_label} -> 「{safe_msg}」 {}",
            format_result_badge(status_code::SUCCESS, None)
        ),
    );
    api_result(status_code::SUCCESS, api::SUCCESS)
}

pub async fn handle_broadcast(
    ctx: &ApiRouteContext<'_>,
    data: &Map<String, Value>,
    client_ip: &str,
) -> ApiResultPayload {
    let Some(message): Option<&str> = data.get("msg").and_then(Value::as_str) else {
        emit_web_log(
            ctx.log_sink,
            log_categories::BROADCAST,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {} {}",
                log_details::INVALID_PARAMS,
                format_result_badge(status_code::BAD_REQUEST, None)
            ),
        );
        return api_result(status_code::BAD_REQUEST, api::NETWORK_PARAM_ERROR);
    };

    let safe_msg: String = sanitize_log_text(message, 48);
    if let Err(err) = ctx.bridge.broadcast_message_to_all(message).await {
        emit_web_log(
            ctx.log_sink,
            log_categories::BROADCAST,
            colors::LIGHT_YELLOW,
            &format!(
                "{client_ip} | {} -> 「{safe_msg}」 {}",
                log_details::BROADCAST_ALL_TARGET,
                format_result_badge(status_code::SERVER_ERROR, Some(&err))
            ),
        );
        return api_result(status_code::SERVER_ERROR, api::SERVER_EXECUTION_ERROR);
    }

    emit_web_log(
        ctx.log_sink,
        log_categories::BROADCAST,
        colors::LIGHT_YELLOW,
        &format!(
            "{client_ip} | {} -> 「{safe_msg}」 {}",
            log_details::BROADCAST_ALL_TARGET,
            format_result_badge(status_code::SUCCESS, None)
        ),
    );
    api_result(status_code::SUCCESS, api::SUCCESS)
}

pub async fn handle_kick_player(
    ctx: &ApiRouteContext<'_>,
    data: &Map<String, Value>,
    client_ip: &str,
) -> ApiResultPayload {
    let Some(target_role): Option<&str> = data.get("player_name").and_then(Value::as_str) else {
        emit_web_log(
            ctx.log_sink,
            log_categories::KICK,
            colors::LIGHT_RED,
            &format!(
                "{client_ip} | {} {}",
                log_details::INVALID_PARAMS,
                format_result_badge(status_code::BAD_REQUEST, None)
            ),
        );
        return api_result(status_code::BAD_REQUEST, api::NETWORK_PARAM_ERROR);
    };

    let role_label: String = sanitize_log_text(role_display(target_role), 24);
    if let Err(err) = ctx.bridge.kick_target_player(target_role).await {
        emit_web_log(
            ctx.log_sink,
            log_categories::KICK,
            colors::LIGHT_RED,
            &format!(
                "{client_ip} | {role_label} {} {}",
                log_details::KICK_VERB,
                format_result_badge(status_code::SERVER_ERROR, Some(&err))
            ),
        );
        return api_result(status_code::SERVER_ERROR, api::SERVER_EXECUTION_ERROR);
    }

    if ctx.auto_broadcast {
        let notice: String = broadcasts::kicked(role_display(target_role));
        if ctx
            .bridge
            .broadcast_message_except(target_role, &notice)
            .await
            .is_err()
        {
            emit_web_log(
                ctx.log_sink,
                log_categories::KICK,
                colors::LIGHT_RED,
                &format!(
                    "{client_ip} | {role_label} {} {}",
                    log_details::KICK_VERB,
                    format_result_badge(status_code::PARTIAL_SUCCESS, None)
                ),
            );
            return api_result(status_code::PARTIAL_SUCCESS, api::BROADCAST_PARTIAL_ERROR);
        }
    }

    emit_web_log(
        ctx.log_sink,
        log_categories::KICK,
        colors::LIGHT_RED,
        &format!(
            "{client_ip} | {role_label} {} {}",
            log_details::KICK_VERB,
            format_result_badge(status_code::SUCCESS, None)
        ),
    );
    api_result(status_code::SUCCESS, api::SUCCESS)
}
