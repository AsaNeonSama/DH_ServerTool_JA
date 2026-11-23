pub mod cli;
pub mod config;
pub mod crypto;
pub mod messages;
pub mod mods;
pub mod network;
pub mod server;
pub mod terminal;
pub mod web_console;

pub mod colors {
    pub const APP_CONSOLE_TITLE: &str = "DH_Server_Tool_JA";

    pub const RESET: &str = "\x1b[0m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const CYAN: &str = "\x1b[36m";
    pub const LIGHT_BLACK: &str = "\x1b[90m";
    pub const LIGHT_RED: &str = "\x1b[91m";
    pub const LIGHT_GREEN: &str = "\x1b[92m";
    pub const LIGHT_YELLOW: &str = "\x1b[93m";
    pub const LIGHT_BLUE: &str = "\x1b[94m";
    pub const LIGHT_MAGENTA: &str = "\x1b[95m";
    pub const LIGHT_CYAN: &str = "\x1b[96m";
    pub const LIGHT_WHITE: &str = "\x1b[97m";

    pub const TAB_ACTIVE_STYLE: &str = "\x1b[1;30;106m";
    pub const TAB_INACTIVE_STYLE: &str = "\x1b[37;100m";
    pub const TAB_BUTTON_STYLE: &str = "\x1b[1;30;47m";
}

pub mod format {
    use chrono::Local;

    #[must_use]
    pub fn format_local_time() -> String {
        Local::now().format("%H:%M:%S").to_string()
    }

    #[must_use]
    pub fn format_log_date_time() -> String {
        Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
    }

    #[must_use]
    pub fn format_seconds(ms: u64) -> String {
        if ms.is_multiple_of(1000) {
            return format!("{}s", ms / 1000);
        }
        let hundredths: u64 = ms.saturating_add(5) / 10;
        let whole: u64 = hundredths / 100;
        let frac: u64 = hundredths % 100;
        if frac == 0 {
            format!("{whole}s")
        } else if frac.is_multiple_of(10) {
            format!("{whole}.{}s", frac / 10)
        } else {
            format!("{whole}.{frac:02}s")
        }
    }

    #[must_use]
    pub fn sanitize_log_text(text: &str, max_chars: usize) -> String {
        let mut clean: String = String::with_capacity(text.len());
        for ch in text.chars() {
            let code: u32 = u32::from(ch);
            if code == 0x09 || code == 0x0a || code == 0x0d {
                clean.push(' ');
                continue;
            }
            if code < 0x20 || (0x7f..=0x9f).contains(&code) {
                continue;
            }
            clean.push(ch);
        }
        let parts: Vec<&str> = clean.split_whitespace().collect();
        let collapsed: String = parts.join(" ");
        if collapsed.chars().count() <= max_chars {
            return collapsed;
        }
        let keep: usize = max_chars.saturating_sub(1).max(1);
        let mut out: String = collapsed.chars().take(keep).collect();
        out.push('…');
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn formats_seconds_matching_ts() {
            assert_eq!(format_seconds(1000), "1s");
            assert_eq!(format_seconds(5000), "5s");
            assert_eq!(format_seconds(250), "0.25s");
            assert_eq!(format_seconds(1500), "1.5s");
            assert_eq!(format_seconds(999), "1s");
        }

        #[test]
        fn sanitizes_control_chars_and_truncates() {
            assert_eq!(sanitize_log_text("a\t\nb\x1b[31mc", 64), "a b[31mc");
            assert_eq!(sanitize_log_text("あいうえお", 4), "あいう…");
            assert_eq!(sanitize_log_text("hello", 5), "hello");
        }
    }
}
