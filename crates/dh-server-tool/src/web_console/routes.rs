use std::iter::Peekable;
use std::str::Chars;

use serde_json::{Map, Value};

pub use crate::consts::web_console::{status_code, HIGH_STAT_WARNING_THRESHOLD};
use crate::consts::web_console::{BONE_DAGGER_ITEM, MAX_SAFE_INT_F64};
use crate::languages;
use crate::structs::languages::{ApiCode, BroadcastEvent, LogCategoryCode, WebLogEvent};
pub use crate::structs::web_console::ApiRouteContext;
use crate::web_console::auth::{api_result, ApiResultPayload};
use crate::web_console::bridge::build_console_broadcast_message;
use crate::web_console::logger::emit_web_log_event;

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

pub async fn handle_console_command(
    ctx: &ApiRouteContext<'_>,
    data: &Map<String, Value>,
    client_ip: &str,
) -> ApiResultPayload {
    let Some(target_role): Option<&str> = data.get("player_name").and_then(Value::as_str) else {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::InvalidParams {
                category: LogCategoryCode::Command,
                client_ip,
            },
        );
        return api_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    };
    let Some(command): Option<&str> = data.get("command").and_then(Value::as_str) else {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::InvalidParams {
                category: LogCategoryCode::Command,
                client_ip,
            },
        );
        return api_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    };

    let trimmed_command: &str = command.trim();
    let (raw_command_name, command_arg): (&str, &str) = match trimmed_command.find(' ') {
        Some(idx) => (&trimmed_command[..idx], trimmed_command[idx + 1..].trim()),
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
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::CommandResult {
                client_ip,
                role_key: target_role,
                command: &normalized_command,
                added_bone_dagger: false,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return api_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
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
            emit_web_log_event(
                ctx.log_sink,
                &WebLogEvent::CommandResult {
                    client_ip,
                    role_key: target_role,
                    command: &normalized_command,
                    added_bone_dagger,
                    status: status_code::PARTIAL_SUCCESS,
                    err_detail: None,
                },
            );
            return api_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
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
                emit_web_log_event(
                    ctx.log_sink,
                    &WebLogEvent::CommandResult {
                        client_ip,
                        role_key: target_role,
                        command: &normalized_command,
                        added_bone_dagger,
                        status: status_code::PARTIAL_SUCCESS,
                        err_detail: None,
                    },
                );
                return api_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
            }
        }
    }

    emit_web_log_event(
        ctx.log_sink,
        &WebLogEvent::CommandResult {
            client_ip,
            role_key: target_role,
            command: &normalized_command,
            added_bone_dagger,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    api_result(status_code::SUCCESS, ApiCode::Success)
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
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::InvalidParams {
                category: LogCategoryCode::Status,
                client_ip,
            },
        );
        return api_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    };
    if is_empty_string {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::InvalidParams {
                category: LogCategoryCode::Status,
                client_ip,
            },
        );
        return api_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    }

    let parsed_value_opt: Option<f64> = js_number_from_value(raw_value);
    let Some(property_value): Option<f64> =
        parsed_value_opt.filter(|v: &f64| v.is_finite() && *v >= 0.0)
    else {
        let raw_text: String = to_string_value(raw_value);
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::StatusInvalidValue {
                client_ip,
                role_key: target_role,
                prop_name: property_name,
                raw_value: &raw_text,
            },
        );
        return api_result(status_code::INVALID_VALUE, ApiCode::InvalidValueError);
    };

    let value_str: String = format_js_number(property_value);
    if let Err(err) = ctx
        .bridge
        .set_human_property(target_role, property_name, property_value)
        .await
    {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::StatusResult {
                client_ip,
                role_key: target_role,
                prop_name: property_name,
                value_text: &value_str,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return api_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
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
            emit_web_log_event(
                ctx.log_sink,
                &WebLogEvent::StatusResult {
                    client_ip,
                    role_key: target_role,
                    prop_name: property_name,
                    value_text: &value_str,
                    status: status_code::PARTIAL_SUCCESS,
                    err_detail: None,
                },
            );
            return api_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
        }
    }

    emit_web_log_event(
        ctx.log_sink,
        &WebLogEvent::StatusResult {
            client_ip,
            role_key: target_role,
            prop_name: property_name,
            value_text: &value_str,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    if property_value > HIGH_STAT_WARNING_THRESHOLD {
        return api_result(status_code::SUCCESS, ApiCode::HighValueWarning);
    }
    api_result(status_code::SUCCESS, ApiCode::Success)
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
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::InvalidParams {
                category: LogCategoryCode::Item,
                client_ip,
            },
        );
        return api_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    };

    let count_str: String = to_string_value(raw_item_count);
    let Some(item_count): Option<i64> = parse_js_int_base10(&count_str).filter(|c: &i64| *c > 0)
    else {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::ItemInvalidCount {
                client_ip,
                role_key: target_role,
                item_key: item_blueprint_path,
                raw_count: &count_str,
            },
        );
        return api_result(status_code::INVALID_VALUE, ApiCode::InvalidValueError);
    };

    if let Err(err) = ctx
        .bridge
        .add_inventory_item(target_role, item_blueprint_path, item_count)
        .await
    {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::ItemResult {
                client_ip,
                role_key: target_role,
                item_key: item_blueprint_path,
                count: item_count,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return api_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
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
            emit_web_log_event(
                ctx.log_sink,
                &WebLogEvent::ItemResult {
                    client_ip,
                    role_key: target_role,
                    item_key: item_blueprint_path,
                    count: item_count,
                    status: status_code::PARTIAL_SUCCESS,
                    err_detail: None,
                },
            );
            return api_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
        }
    }

    emit_web_log_event(
        ctx.log_sink,
        &WebLogEvent::ItemResult {
            client_ip,
            role_key: target_role,
            item_key: item_blueprint_path,
            count: item_count,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    api_result(status_code::SUCCESS, ApiCode::Success)
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
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::InvalidParams {
                category: LogCategoryCode::Storage,
                client_ip,
            },
        );
        return api_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    };

    let limit_str: String = to_string_value(raw_limit);
    let Some(storage_limit): Option<i64> = parse_js_int_base10(&limit_str).filter(|l: &i64| *l > 0)
    else {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::StorageInvalidValue {
                client_ip,
                role_key: target_role,
                raw_limit: &limit_str,
            },
        );
        return api_result(status_code::INVALID_VALUE, ApiCode::InvalidValueError);
    };

    if let Err(err) = ctx
        .bridge
        .set_player_storage_limit(target_role, storage_limit)
        .await
    {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::StorageResult {
                client_ip,
                role_key: target_role,
                limit: storage_limit,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return api_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
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
            emit_web_log_event(
                ctx.log_sink,
                &WebLogEvent::StorageResult {
                    client_ip,
                    role_key: target_role,
                    limit: storage_limit,
                    status: status_code::PARTIAL_SUCCESS,
                    err_detail: None,
                },
            );
            return api_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
        }
    }

    emit_web_log_event(
        ctx.log_sink,
        &WebLogEvent::StorageResult {
            client_ip,
            role_key: target_role,
            limit: storage_limit,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    api_result(status_code::SUCCESS, ApiCode::Success)
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
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::InvalidParams {
                category: LogCategoryCode::Notice,
                client_ip,
            },
        );
        return api_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    };

    if let Err(err) = ctx.bridge.send_player_message(target_role, message).await {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::NoticeResult {
                client_ip,
                role_key: target_role,
                message,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return api_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
    }

    emit_web_log_event(
        ctx.log_sink,
        &WebLogEvent::NoticeResult {
            client_ip,
            role_key: target_role,
            message,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    api_result(status_code::SUCCESS, ApiCode::Success)
}

pub async fn handle_broadcast(
    ctx: &ApiRouteContext<'_>,
    data: &Map<String, Value>,
    client_ip: &str,
) -> ApiResultPayload {
    let Some(message): Option<&str> = data.get("msg").and_then(Value::as_str) else {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::InvalidParams {
                category: LogCategoryCode::Broadcast,
                client_ip,
            },
        );
        return api_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    };

    if let Err(err) = ctx.bridge.broadcast_message_to_all(message).await {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::BroadcastResult {
                client_ip,
                message,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return api_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
    }

    emit_web_log_event(
        ctx.log_sink,
        &WebLogEvent::BroadcastResult {
            client_ip,
            message,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    api_result(status_code::SUCCESS, ApiCode::Success)
}

pub async fn handle_kick_player(
    ctx: &ApiRouteContext<'_>,
    data: &Map<String, Value>,
    client_ip: &str,
) -> ApiResultPayload {
    let Some(target_role): Option<&str> = data.get("player_name").and_then(Value::as_str) else {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::InvalidParams {
                category: LogCategoryCode::Kick,
                client_ip,
            },
        );
        return api_result(status_code::BAD_REQUEST, ApiCode::NetworkParamError);
    };

    if let Err(err) = ctx.bridge.kick_target_player(target_role).await {
        emit_web_log_event(
            ctx.log_sink,
            &WebLogEvent::KickResult {
                client_ip,
                role_key: target_role,
                status: status_code::SERVER_ERROR,
                err_detail: Some(&err),
            },
        );
        return api_result(status_code::SERVER_ERROR, ApiCode::ServerExecutionError);
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
            emit_web_log_event(
                ctx.log_sink,
                &WebLogEvent::KickResult {
                    client_ip,
                    role_key: target_role,
                    status: status_code::PARTIAL_SUCCESS,
                    err_detail: None,
                },
            );
            return api_result(status_code::PARTIAL_SUCCESS, ApiCode::BroadcastPartialError);
        }
    }

    emit_web_log_event(
        ctx.log_sink,
        &WebLogEvent::KickResult {
            client_ip,
            role_key: target_role,
            status: status_code::SUCCESS,
            err_detail: None,
        },
    );
    api_result(status_code::SUCCESS, ApiCode::Success)
}
