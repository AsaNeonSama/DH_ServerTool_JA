use std::fmt::Write as _;

use crate::colors;
use crate::messages::ja::terminal as term_msg;
use crate::terminal::display::{
    count_visual_rows, display_width, measure_prompt_cursor, select_visible_lines,
    CursorMeasurement, VisibleLinesResult,
};
use crate::web_console::WebConsoleDisplayInfo;

pub const TAB_HEADER_ROWS: usize = 2;
pub const MAX_WEB_LOG_LINES: usize = 512;
pub const MAX_SERVER_TAB_LINES: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTabId {
    Web,
    Server,
}

/// One-based inclusive hit-test rectangle on a single terminal row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitRegion {
    pub row: usize,
    pub start_col: usize,
    pub end_col: usize,
}

impl HitRegion {
    #[must_use]
    pub fn contains(self, col_1based: usize, row_1based: usize) -> bool {
        row_1based == self.row && col_1based >= self.start_col && col_1based <= self.end_col
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyFeedbackState {
    pub text: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabBarLayout {
    pub row1: String,
    pub row2: String,
    pub tab1_hit: HitRegion,
    pub tab2_hit: HitRegion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebTabLayout {
    pub lines: Vec<String>,
    pub public_btn_hit: Option<HitRegion>,
    pub local_btn_hit: Option<HitRegion>,
}

pub struct FrameRenderInput<'a> {
    pub term_cols: usize,
    pub term_rows: usize,
    pub active_tab: TerminalTabId,
    pub web_console_info: Option<&'a WebConsoleDisplayInfo>,
    pub copy_feedback: Option<&'a CopyFeedbackState>,
    pub web_log_lines: &'a [String],
    pub web_log_scroll_offset: usize,
    pub server_lines: &'a [String],
    pub server_scroll_offset: usize,
    pub status_line: Option<&'a str>,
    pub prompt_text: Option<&'a str>,
    pub prompt_input: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedFrame {
    pub ansi_frame: String,
    pub tab1_hit: HitRegion,
    pub tab2_hit: HitRegion,
    pub public_btn_hit: Option<HitRegion>,
    pub local_btn_hit: Option<HitRegion>,
    pub clamped_server_offset: usize,
    pub clamped_web_offset: usize,
}

#[must_use]
pub fn build_tab_bar(
    term_cols: usize,
    active_tab: TerminalTabId,
    web_console_info: Option<&WebConsoleDisplayInfo>,
    server_scroll_offset: usize,
) -> TabBarLayout {
    let port_suffix: String = web_console_info
        .map_or_else(String::new, |info: &WebConsoleDisplayInfo| {
            format!(" ({})", info.port)
        });
    let tab1_text: String = term_msg::tab1_title(&port_suffix);
    let tab2_text: &str = term_msg::TAB2_TITLE;
    let sep_text: &str = " │ ";
    let hint_text: &str = term_msg::TAB_SWITCH_HINT;

    let tab1_width: usize = display_width(&tab1_text);
    let sep_width: usize = display_width(sep_text);
    let tab2_width: usize = display_width(tab2_text);
    let hint_width: usize = display_width(hint_text);

    let tab1_start_col: usize = 2;
    let tab1_end_col: usize = tab1_start_col + tab1_width.saturating_sub(1);
    let tab2_start_col: usize = tab1_end_col + sep_width + 1;
    let tab2_end_col: usize = tab2_start_col + tab2_width.saturating_sub(1);

    let tab1_style: &str = if active_tab == TerminalTabId::Web {
        colors::TAB_ACTIVE_STYLE
    } else {
        colors::TAB_INACTIVE_STYLE
    };
    let tab2_style: &str = if active_tab == TerminalTabId::Server {
        colors::TAB_ACTIVE_STYLE
    } else {
        colors::TAB_INACTIVE_STYLE
    };

    let base_width: usize = 1 + tab1_width + sep_width + tab2_width;
    let hint_segment: String = if base_width + hint_width <= term_cols {
        format!("{}{hint_text}{}", colors::LIGHT_BLACK, colors::RESET)
    } else {
        String::new()
    };

    let row1: String = format!(
        " {tab1_style}{tab1_text}{}{}{sep_text}{}{tab2_style}{tab2_text}{}{hint_segment}",
        colors::RESET,
        colors::LIGHT_BLACK,
        colors::RESET,
        colors::RESET,
    );

    let mut row2: String = format!(
        "{}{}{}",
        colors::LIGHT_BLACK,
        "─".repeat(term_cols),
        colors::RESET
    );
    if active_tab == TerminalTabId::Server && server_scroll_offset > 0 {
        let scroll_badge: String = term_msg::server_scroll_badge(server_scroll_offset);
        let badge_width: usize = display_width(&scroll_badge);
        if badge_width + 4 < term_cols {
            let left_dashes: usize = term_cols.saturating_sub(badge_width + 2).max(2);
            row2 = format!(
                "{}{}{}{scroll_badge}{}──{}",
                colors::LIGHT_BLACK,
                "─".repeat(left_dashes),
                colors::LIGHT_YELLOW,
                colors::LIGHT_BLACK,
                colors::RESET
            );
        }
    }

    TabBarLayout {
        row1,
        row2,
        tab1_hit: HitRegion {
            row: 1,
            start_col: tab1_start_col,
            end_col: tab1_end_col,
        },
        tab2_hit: HitRegion {
            row: 1,
            start_col: tab2_start_col,
            end_col: tab2_end_col,
        },
    }
}

#[must_use]
pub fn build_web_tab_layout(
    term_cols: usize,
    web_console_info: Option<&WebConsoleDisplayInfo>,
    copy_feedback: Option<&CopyFeedbackState>,
    has_pending_server_action: bool,
) -> WebTabLayout {
    let Some(info): Option<&WebConsoleDisplayInfo> = web_console_info else {
        return WebTabLayout {
            lines: Vec::new(),
            public_btn_hit: None,
            local_btn_hit: None,
        };
    };

    let broadcast_text: &str = if info.auto_broadcast {
        term_msg::TOGGLE_ON
    } else {
        term_msg::TOGGLE_OFF
    };

    let prefix_lines: Vec<String> = vec![
        format!("{} {}", colors::LIGHT_CYAN, term_msg::WEB_SETTINGS_HEADER),
        format!(
            "{} * {} {}{} {}{} {}{broadcast_text}",
            colors::LIGHT_WHITE,
            term_msg::WEB_TCP_PORT_LABEL,
            colors::LIGHT_GREEN,
            info.port,
            colors::LIGHT_WHITE,
            term_msg::WEB_AUTO_BROADCAST_LABEL,
            colors::LIGHT_GREEN,
        ),
        String::new(),
        format!("{} {}", colors::LIGHT_CYAN, term_msg::WEB_STARTED_HEADER),
        format!(
            "{} * {} {}",
            colors::LIGHT_BLACK,
            term_msg::PUBLIC_URL_LABEL,
            info.public_url
        ),
        format!(
            "{} * {} {}",
            colors::LIGHT_BLACK,
            term_msg::LOCAL_URL_LABEL,
            info.local_url
        ),
        format!(
            "{} * {}{}{}{}{}",
            colors::LIGHT_YELLOW,
            term_msg::PSK_NOTICE_LEAD,
            colors::LIGHT_WHITE,
            term_msg::PSK_NOTICE_BODY,
            colors::LIGHT_MAGENTA,
            term_msg::PSK_NOTICE_WARN
        ),
        format!(
            "{} * {}",
            colors::LIGHT_BLACK,
            term_msg::internal_bridge_port_summary(info.console_patch_port, broadcast_text)
        ),
        format!("{} {}", colors::LIGHT_CYAN, term_msg::WEB_STARTED_FOOTER),
        String::new(),
    ];

    let mut visual_row_cursor: usize = TAB_HEADER_ROWS + 1;
    for line in &prefix_lines {
        visual_row_cursor += count_visual_rows(line, term_cols);
    }

    let pub_btn_label: &str = term_msg::COPY_PUBLIC_BTN_LABEL;
    let loc_btn_label: &str = term_msg::COPY_LOCAL_BTN_LABEL;
    let left_pad: &str = " ";
    let gap: &str = "  ";

    let pub_start_col: usize = display_width(left_pad) + 1;
    let pub_end_col: usize = pub_start_col + display_width(pub_btn_label).saturating_sub(1);
    let loc_start_col: usize = pub_end_col + display_width(gap) + 1;
    let loc_end_col: usize = loc_start_col + display_width(loc_btn_label).saturating_sub(1);

    let button_line: String = format!(
        "{left_pad}{}{pub_btn_label}{}{gap}{}{loc_btn_label}{}",
        colors::TAB_BUTTON_STYLE,
        colors::RESET,
        colors::TAB_BUTTON_STYLE,
        colors::RESET
    );

    let mut lines: Vec<String> = prefix_lines;
    lines.push(button_line);
    if let Some(fb) = copy_feedback {
        let fb_color: &str = if fb.is_error {
            colors::RED
        } else {
            colors::LIGHT_GREEN
        };
        lines.push(format!("{fb_color}{}", fb.text));
    }
    lines.push(format!(
        "{} * {}",
        colors::LIGHT_BLACK,
        term_msg::COPY_GUIDE_HINT
    ));
    if has_pending_server_action {
        lines.push(format!(
            "{} * {}",
            colors::LIGHT_YELLOW,
            term_msg::SERVER_PENDING_INPUT_HINT
        ));
    } else {
        lines.push(format!(
            "{} * {}",
            colors::LIGHT_BLACK,
            term_msg::SWITCH_TO_SERVER_HINT
        ));
    }

    let button_row_fits: bool = loc_end_col <= term_cols;
    WebTabLayout {
        lines,
        public_btn_hit: button_row_fits.then_some(HitRegion {
            row: visual_row_cursor,
            start_col: pub_start_col,
            end_col: pub_end_col,
        }),
        local_btn_hit: button_row_fits.then_some(HitRegion {
            row: visual_row_cursor,
            start_col: loc_start_col,
            end_col: loc_end_col,
        }),
    }
}

#[must_use]
pub fn build_web_log_bar(term_cols: usize, total_lines: usize, scroll_pos: usize) -> String {
    let left_text: String = term_msg::web_log_bar_title(total_lines, MAX_WEB_LOG_LINES);
    let (right_text, right_color): (String, &str) = if scroll_pos > 0 {
        (
            term_msg::web_log_bar_scrolled(scroll_pos),
            colors::LIGHT_YELLOW,
        )
    } else {
        (
            term_msg::WEB_LOG_BAR_CONTROLS_HINT.to_string(),
            colors::LIGHT_BLACK,
        )
    };

    let left_lead: usize = 2;
    let right_trail: usize = 2;
    let left_width: usize = display_width(&left_text);
    let right_width: usize = display_width(&right_text);

    if left_lead + left_width + 2 + right_width + right_trail <= term_cols {
        let mid_dashes: usize = term_cols - left_lead - left_width - right_width - right_trail;
        return format!(
            "{}{}{}{left_text}{}{}{right_color}{right_text}{}{}{}",
            colors::LIGHT_BLACK,
            "─".repeat(left_lead),
            colors::LIGHT_CYAN,
            colors::LIGHT_BLACK,
            "─".repeat(mid_dashes),
            colors::LIGHT_BLACK,
            "─".repeat(right_trail),
            colors::RESET
        );
    }

    if left_lead + left_width + right_trail <= term_cols {
        let right_dashes: usize = term_cols - left_lead - left_width;
        return format!(
            "{}{}{}{left_text}{}{}{}",
            colors::LIGHT_BLACK,
            "─".repeat(left_lead),
            colors::LIGHT_CYAN,
            colors::LIGHT_BLACK,
            "─".repeat(right_dashes),
            colors::RESET
        );
    }

    format!(
        "{}{}{}",
        colors::LIGHT_BLACK,
        "─".repeat(term_cols),
        colors::RESET
    )
}

/// Builds a complete atomic Synchronized Output Mode frame (`\x1b[?2026h ... \x1b[?2026l`).
#[must_use]
pub fn render_frame_buffer(input: &FrameRenderInput<'_>) -> RenderedFrame {
    let term_cols: usize = input.term_cols.max(40);
    let term_rows: usize = input.term_rows.max(10);
    let body_height: usize = term_rows.saturating_sub(TAB_HEADER_ROWS).max(3);

    let mut body_lines: Vec<String> = Vec::new();
    let mut cursor_pos: Option<(usize, usize)> = None;
    let mut public_btn_hit: Option<HitRegion> = None;
    let mut local_btn_hit: Option<HitRegion> = None;
    let mut clamped_server_offset: usize = input.server_scroll_offset;
    let mut clamped_web_offset: usize = input.web_log_scroll_offset;

    match input.active_tab {
        TerminalTabId::Web => {
            let web_layout: WebTabLayout = build_web_tab_layout(
                term_cols,
                input.web_console_info,
                input.copy_feedback,
                input.prompt_text.is_some() || input.status_line.is_some(),
            );
            public_btn_hit = web_layout.public_btn_hit;
            local_btn_hit = web_layout.local_btn_hit;

            let mut used_rows: usize = 0;
            for line in web_layout.lines {
                let r: usize = count_visual_rows(&line, term_cols);
                if used_rows + r > body_height {
                    break;
                }
                body_lines.push(format!("{line}{}", colors::RESET));
                used_rows += r;
            }

            let rows_after_header: usize = body_height.saturating_sub(used_rows);
            if rows_after_header >= 3 {
                let available_log_rows: usize = rows_after_header - 2;
                let selected: VisibleLinesResult = select_visible_lines(
                    input.web_log_lines,
                    input.web_log_scroll_offset,
                    available_log_rows,
                    term_cols,
                );
                clamped_web_offset = selected.clamped_offset;

                body_lines.push(String::new());
                body_lines.push(build_web_log_bar(
                    term_cols,
                    input.web_log_lines.len(),
                    clamped_web_offset,
                ));

                if selected.visible_lines.is_empty() {
                    body_lines.push(format!(
                        "{} {}{}",
                        colors::LIGHT_BLACK,
                        term_msg::WEB_LOG_EMPTY_PLACEHOLDER,
                        colors::RESET
                    ));
                } else {
                    for log_line in selected.visible_lines {
                        body_lines.push(format!("{log_line}{}", colors::RESET));
                    }
                }
            }
        }
        TerminalTabId::Server => {
            let status_visual_rows: usize = input
                .status_line
                .map_or(0, |line: &str| count_visual_rows(line, term_cols));

            let prompt_rendered: Option<String> = input
                .prompt_text
                .map(|p: &str| format!("{p}{}{}", colors::RESET, input.prompt_input));
            let prompt_cursor: Option<CursorMeasurement> = prompt_rendered
                .as_deref()
                .map(|line: &str| measure_prompt_cursor(line, term_cols));
            let prompt_visual_rows: usize =
                prompt_cursor.map_or(0, |m: CursorMeasurement| m.visual_rows);

            let reserved_rows: usize = status_visual_rows + prompt_visual_rows;
            let available_log_rows: usize = body_height.saturating_sub(reserved_rows).max(1);
            let selected: VisibleLinesResult = select_visible_lines(
                input.server_lines,
                input.server_scroll_offset,
                available_log_rows,
                term_cols,
            );
            clamped_server_offset = selected.clamped_offset;

            let mut current_visual_row: usize = TAB_HEADER_ROWS + 1;
            for line in selected.visible_lines {
                current_visual_row += count_visual_rows(&line, term_cols);
                body_lines.push(format!("{line}{}", colors::RESET));
            }

            if let Some(status) = input.status_line {
                if current_visual_row + status_visual_rows.saturating_sub(1) <= term_rows {
                    body_lines.push(format!("{status}{}", colors::RESET));
                    current_visual_row += status_visual_rows;
                }
            }

            if let (Some(prompt_line), Some(meas)) = (prompt_rendered, prompt_cursor) {
                if current_visual_row <= term_rows {
                    let prompt_start_row: usize = current_visual_row;
                    body_lines.push(prompt_line);
                    let cursor_row: usize = (prompt_start_row + meas.row_offset).min(term_rows);
                    cursor_pos = Some((cursor_row, meas.cursor_col));
                }
            }
        }
    }

    let tab_bar: TabBarLayout = build_tab_bar(
        term_cols,
        input.active_tab,
        input.web_console_info,
        clamped_server_offset,
    );

    let mut frame: String = String::with_capacity(4096);
    frame.push_str("\x1b[?2026h\x1b[?25l\x1b[H");
    frame.push_str(&tab_bar.row1);
    frame.push_str("\x1b[K\r\n");
    frame.push_str(&tab_bar.row2);
    frame.push_str("\x1b[K\r\n\x1b[J");

    if !body_lines.is_empty() {
        frame.push_str(&body_lines.join("\r\n"));
    }

    if let Some((row, col)) = cursor_pos {
        let _ = write!(frame, "\x1b[{row};{col}H\x1b[?25h");
    }
    frame.push_str("\x1b[?2026l");

    RenderedFrame {
        ansi_frame: frame,
        tab1_hit: tab_bar.tab1_hit,
        tab2_hit: tab_bar.tab2_hit,
        public_btn_hit,
        local_btn_hit,
        clamped_server_offset,
        clamped_web_offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_web_info() -> WebConsoleDisplayInfo {
        WebConsoleDisplayInfo {
            port: 7891,
            console_patch_port: 7893,
            auto_broadcast: true,
            public_url: "http://203.0.113.10:7891/dh-console/auth#testpsk".to_string(),
            local_url: "http://127.0.0.1:7891/dh-console/auth#testpsk".to_string(),
        }
    }

    #[test]
    fn tab_bar_computes_non_overlapping_hit_regions() {
        let info = sample_web_info();
        let layout = build_tab_bar(120, TerminalTabId::Server, Some(&info), 0);
        assert_eq!(layout.tab1_hit.row, 1);
        assert_eq!(layout.tab2_hit.row, 1);
        assert_eq!(layout.tab1_hit.start_col, 2);
        assert!(layout.tab1_hit.end_col < layout.tab2_hit.start_col);
        assert!(layout.tab1_hit.contains(2, 1));
        assert!(layout.tab2_hit.contains(layout.tab2_hit.start_col, 1));
        assert!(!layout.tab1_hit.contains(layout.tab2_hit.start_col, 1));
    }

    #[test]
    fn web_tab_computes_clickable_clipboard_buttons_and_sync_frame() {
        let info = sample_web_info();
        let web_logs = vec![" [12:00:00] [起動] test".to_string()];
        let rendered = render_frame_buffer(&FrameRenderInput {
            term_cols: 120,
            term_rows: 30,
            active_tab: TerminalTabId::Web,
            web_console_info: Some(&info),
            copy_feedback: None,
            web_log_lines: &web_logs,
            web_log_scroll_offset: 0,
            server_lines: &[],
            server_scroll_offset: 0,
            status_line: None,
            prompt_text: None,
            prompt_input: "",
        });

        assert!(rendered.ansi_frame.starts_with("\x1b[?2026h"));
        assert!(rendered.ansi_frame.ends_with("\x1b[?2026l"));
        let pub_btn = rendered.public_btn_hit.expect("public button hit region");
        let loc_btn = rendered.local_btn_hit.expect("local button hit region");
        assert_eq!(pub_btn.row, loc_btn.row);
        assert!(pub_btn.end_col < loc_btn.start_col);
    }

    #[test]
    fn server_tab_places_cursor_for_active_prompt_and_scroll_badge() {
        let info = sample_web_info();
        let server_lines: Vec<String> = (1..=30).map(|i| format!("log {i}")).collect();
        let rendered = render_frame_buffer(&FrameRenderInput {
            term_cols: 100,
            term_rows: 20,
            active_tab: TerminalTabId::Server,
            web_console_info: Some(&info),
            copy_feedback: None,
            web_log_lines: &[],
            web_log_scroll_offset: 0,
            server_lines: &server_lines,
            server_scroll_offset: 4,
            status_line: None,
            prompt_text: Some(" * 選択肢: "),
            prompt_input: "1,7",
        });

        assert!(rendered.ansi_frame.contains("\x1b[?25h"));
        assert!(rendered.ansi_frame.contains("▲ 4行上を表示中"));
        assert!(rendered.public_btn_hit.is_none());
        assert!(rendered.local_btn_hit.is_none());
    }
}
