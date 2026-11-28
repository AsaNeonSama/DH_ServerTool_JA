pub mod config;
pub mod console;
pub mod consts;
pub mod gui;
pub mod languages;
pub mod mods;
pub mod server;
pub mod structs;

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
    pub fn format_log_filename_timestamp() -> String {
        Local::now().format("%Y%m%d_%H%M%S").to_string()
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
        if max_chars == 0 {
            return String::new();
        }
        let mut out: String = String::with_capacity(text.len().min(max_chars.saturating_mul(4)));
        let mut char_count: usize = 0;
        let mut in_whitespace: bool = true;
        let mut truncated: bool = false;

        for ch in text.chars() {
            let code: u32 = u32::from(ch);
            let mapped: char = if code == 0x09 || code == 0x0a || code == 0x0d {
                ' '
            } else if code < 0x20 || (0x7f..=0x9f).contains(&code) {
                continue;
            } else {
                ch
            };

            if mapped.is_whitespace() {
                in_whitespace = true;
                continue;
            }

            if in_whitespace && char_count > 0 {
                if char_count >= max_chars {
                    truncated = true;
                    break;
                }
                out.push(' ');
                char_count += 1;
            }
            in_whitespace = false;

            if char_count >= max_chars {
                truncated = true;
                break;
            }
            out.push(mapped);
            char_count += 1;
        }

        if !truncated {
            return out;
        }
        if max_chars == 1 {
            return "…".to_string();
        }
        let keep: usize = max_chars.saturating_sub(1);
        let mut trimmed: String = out.chars().take(keep).collect();
        trimmed.push('…');
        trimmed
    }
}
