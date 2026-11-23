pub mod cli;
pub mod config;
pub mod consts;
pub mod crypto;
pub mod languages;
pub mod mods;
pub mod network;
pub mod server;
pub mod structs;
pub mod terminal;
pub mod web_console;

pub use consts::colors;

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
}
