use std::sync::Arc;

use crate::colors;
use crate::format::{format_local_time, sanitize_log_text};
use crate::messages::ja::web_console::{actor_display_name, result_badges};
use crate::web_console::routes::status_code;

pub const MAX_LOG_TEXT_CHARS: usize = 64;

/// Callback sink for formatted Tab 1 (Web Console) log lines.
pub type WebLogSink = Arc<dyn Fn(String) + Send + Sync>;

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
        return format!("{}{}", colors::LIGHT_GREEN, result_badges::SENT);
    }
    if status == status_code::PARTIAL_SUCCESS {
        return format!("{}{}", colors::LIGHT_YELLOW, result_badges::PARTIAL_FAILURE);
    }
    let suffix: String = match err_detail {
        Some(detail) if !detail.is_empty() => format!(": {}", sanitize_log_text(detail, 36)),
        _ => String::new(),
    };
    if status == status_code::SERVER_ERROR {
        format!("{}{}", colors::RED, result_badges::send_failed(&suffix))
    } else {
        format!("{}{}", colors::RED, result_badges::invalid(&suffix))
    }
}

pub fn emit_web_log(sink: &WebLogSink, category: &str, category_color: &str, detail: &str) {
    let line: String = format!(
        " {}[{}] {}[{}] {}{}",
        colors::LIGHT_BLACK,
        format_local_time(),
        category_color,
        category,
        colors::LIGHT_WHITE,
        detail,
    );
    (sink)(line);
}

#[must_use]
pub fn format_console_command_preview(command: &str) -> String {
    let (cmd_name, cmd_arg): (&str, &str) = match command.find(' ') {
        Some(idx) => (&command[..idx], command[idx + 1..].trim()),
        None => (command, ""),
    };
    if cmd_name == "summon" && !cmd_arg.is_empty() {
        if let Some(known_name) = actor_display_name(cmd_arg) {
            return format!("summon {known_name}");
        }
    }
    sanitize_log_text(command, MAX_LOG_TEXT_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_client_ips() {
        assert_eq!(normalize_client_ip(None), "unknown");
        assert_eq!(normalize_client_ip(Some("")), "unknown");
        assert_eq!(normalize_client_ip(Some("::1")), "127.0.0.1");
        assert_eq!(
            normalize_client_ip(Some("::ffff:192.168.1.5")),
            "192.168.1.5"
        );
        assert_eq!(normalize_client_ip(Some(" 203.0.113.9\r\n")), "203.0.113.9");
    }

    #[test]
    fn formats_console_command_preview() {
        assert_eq!(
            format_console_command_preview("summon BP_Predator_Bear_C"),
            "summon 熊"
        );
        assert_eq!(
            format_console_command_preview("setisthrall 1"),
            "setisthrall 1"
        );
    }
}
