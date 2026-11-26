use std::fmt::Write as _;

use crate::colors;
pub use crate::consts::terminal::{MAX_SERVER_TAB_LINES, MAX_WEB_LOG_LINES, TAB_HEADER_ROWS};
use crate::languages;
use crate::structs::languages::{TerminalMsg, TerminalMsgCode};
pub use crate::structs::terminal::{
    CopyFeedbackState, FrameRenderInput, HitRegion, RenderedFrame, TabBarLayout, TerminalTabId,
    WebTabLayout,
};
use crate::terminal::display::{
    count_visual_rows, display_width, measure_prompt_cursor, select_visible_lines,
    CursorMeasurement, VisibleLinesResult,
};
use crate::web_console::WebConsoleDisplayInfo;

impl HitRegion {
    #[must_use]
    pub fn contains(self, col_1based: usize, row_1based: usize) -> bool {
        row_1based == self.row && col_1based >= self.start_col && col_1based <= self.end_col
    }
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
    let tab1_text: String = languages::render_terminal(&TerminalMsg::Tab1Title {
        port_suffix: &port_suffix,
    });
    let tab2_text: &str = languages::terminal_text(TerminalMsgCode::Tab2Title);
    let sep_text: &str = " │ ";
    let hint_text: &str = languages::terminal_text(TerminalMsgCode::TabSwitchHint);

    let tab1_width: usize = display_width(&tab1_text);
    let sep_width: usize = display_width(sep_text);
    let tab2_width: usize = display_width(tab2_text);
    let hint_width: usize = display_width(hint_text);

    let tab1_start_col: usize = 2;
    let tab1_end_col: usize = tab1_start_col.saturating_add(tab1_width.saturating_sub(1));
    let tab2_start_col: usize = tab1_end_col.saturating_add(sep_width).saturating_add(1);
    let tab2_end_col: usize = tab2_start_col.saturating_add(tab2_width.saturating_sub(1));

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

    let base_width: usize = 1usize
        .saturating_add(tab1_width)
        .saturating_add(sep_width)
        .saturating_add(tab2_width);
    let hint_segment: String = if base_width.saturating_add(hint_width) <= term_cols {
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
        let scroll_badge: String = languages::render_terminal(&TerminalMsg::ServerScrollBadge {
            offset: server_scroll_offset,
        });
        let badge_width: usize = display_width(&scroll_badge);
        if badge_width.saturating_add(4) < term_cols {
            let left_dashes: usize = term_cols
                .saturating_sub(badge_width.saturating_add(2))
                .max(2);
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
        languages::terminal_text(TerminalMsgCode::ToggleOn)
    } else {
        languages::terminal_text(TerminalMsgCode::ToggleOff)
    };

    let prefix_lines: Vec<String> = vec![
        format!(
            "{} {}",
            colors::LIGHT_CYAN,
            languages::terminal_text(TerminalMsgCode::WebSettingsHeader)
        ),
        format!(
            "{} * {} {}{} {}{} {}{broadcast_text}",
            colors::LIGHT_WHITE,
            languages::terminal_text(TerminalMsgCode::WebTcpPortLabel),
            colors::LIGHT_GREEN,
            info.port,
            colors::LIGHT_WHITE,
            languages::terminal_text(TerminalMsgCode::WebAutoBroadcastLabel),
            colors::LIGHT_GREEN,
        ),
        String::new(),
        format!(
            "{} {}",
            colors::LIGHT_CYAN,
            languages::terminal_text(TerminalMsgCode::WebStartedHeader)
        ),
        format!(
            "{} * {} {}",
            colors::LIGHT_BLACK,
            languages::terminal_text(TerminalMsgCode::PublicUrlLabel),
            info.public_url
        ),
        format!(
            "{} * {} {}",
            colors::LIGHT_BLACK,
            languages::terminal_text(TerminalMsgCode::LocalUrlLabel),
            info.local_url
        ),
        format!(
            "{} * {}{}{}{}{}",
            colors::LIGHT_YELLOW,
            languages::terminal_text(TerminalMsgCode::PskNoticeLead),
            colors::LIGHT_WHITE,
            languages::terminal_text(TerminalMsgCode::PskNoticeBody),
            colors::LIGHT_MAGENTA,
            languages::terminal_text(TerminalMsgCode::PskNoticeWarn)
        ),
        format!(
            "{} * {}",
            colors::LIGHT_BLACK,
            languages::render_terminal(&TerminalMsg::InternalBridgePortSummary {
                port: info.console_patch_port,
                auto_broadcast: info.auto_broadcast,
            })
        ),
        format!(
            "{} {}",
            colors::LIGHT_CYAN,
            languages::terminal_text(TerminalMsgCode::WebStartedFooter)
        ),
        String::new(),
    ];

    let mut visual_row_cursor: usize = TAB_HEADER_ROWS.saturating_add(1);
    for line in &prefix_lines {
        visual_row_cursor = visual_row_cursor.saturating_add(count_visual_rows(line, term_cols));
    }

    let pub_btn_label: &str = languages::terminal_text(TerminalMsgCode::CopyPublicBtnLabel);
    let loc_btn_label: &str = languages::terminal_text(TerminalMsgCode::CopyLocalBtnLabel);
    let left_pad: &str = " ";
    let gap: &str = "  ";

    let pub_start_col: usize = display_width(left_pad).saturating_add(1);
    let pub_end_col: usize =
        pub_start_col.saturating_add(display_width(pub_btn_label).saturating_sub(1));
    let loc_start_col: usize = pub_end_col
        .saturating_add(display_width(gap))
        .saturating_add(1);
    let loc_end_col: usize =
        loc_start_col.saturating_add(display_width(loc_btn_label).saturating_sub(1));

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
        languages::terminal_text(TerminalMsgCode::CopyGuideHint)
    ));
    if has_pending_server_action {
        lines.push(format!(
            "{} * {}",
            colors::LIGHT_YELLOW,
            languages::terminal_text(TerminalMsgCode::ServerPendingInputHint)
        ));
    } else {
        lines.push(format!(
            "{} * {}",
            colors::LIGHT_BLACK,
            languages::terminal_text(TerminalMsgCode::SwitchToServerHint)
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
    let left_text: String = languages::render_terminal(&TerminalMsg::WebLogBarTitle {
        total_lines,
        max_lines: MAX_WEB_LOG_LINES,
    });
    let (right_text, right_color): (String, &str) = if scroll_pos > 0 {
        (
            languages::render_terminal(&TerminalMsg::WebLogBarScrolled { scroll_pos }),
            colors::LIGHT_YELLOW,
        )
    } else {
        (
            languages::terminal_text(TerminalMsgCode::WebLogBarControlsHint).to_string(),
            colors::LIGHT_BLACK,
        )
    };

    let left_lead: usize = 2;
    let right_trail: usize = 2;
    let left_width: usize = display_width(&left_text);
    let right_width: usize = display_width(&right_text);

    let full_min_cols: usize = left_lead
        .saturating_add(left_width)
        .saturating_add(2)
        .saturating_add(right_width)
        .saturating_add(right_trail);
    if full_min_cols <= term_cols {
        let mid_dashes: usize = term_cols
            .saturating_sub(left_lead)
            .saturating_sub(left_width)
            .saturating_sub(right_width)
            .saturating_sub(right_trail);
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

    let compact_min_cols: usize = left_lead
        .saturating_add(left_width)
        .saturating_add(right_trail);
    if compact_min_cols <= term_cols {
        let right_dashes: usize = term_cols
            .saturating_sub(left_lead)
            .saturating_sub(left_width);
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
                if used_rows.saturating_add(r) > body_height {
                    break;
                }
                body_lines.push(format!("{line}{}", colors::RESET));
                used_rows = used_rows.saturating_add(r);
            }

            let rows_after_header: usize = body_height.saturating_sub(used_rows);
            if rows_after_header >= 3 {
                let available_log_rows: usize = rows_after_header.saturating_sub(2);
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
                        languages::terminal_text(TerminalMsgCode::WebLogEmptyPlaceholder),
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

            let reserved_rows: usize = status_visual_rows.saturating_add(prompt_visual_rows);
            let available_log_rows: usize = body_height.saturating_sub(reserved_rows).max(1);
            let selected: VisibleLinesResult = select_visible_lines(
                input.server_lines,
                input.server_scroll_offset,
                available_log_rows,
                term_cols,
            );
            clamped_server_offset = selected.clamped_offset;

            let mut current_visual_row: usize = TAB_HEADER_ROWS.saturating_add(1);
            for line in selected.visible_lines {
                current_visual_row =
                    current_visual_row.saturating_add(count_visual_rows(&line, term_cols));
                body_lines.push(format!("{line}{}", colors::RESET));
            }

            if let Some(status) = input.status_line {
                if current_visual_row.saturating_add(status_visual_rows.saturating_sub(1))
                    <= term_rows
                {
                    body_lines.push(format!("{status}{}", colors::RESET));
                    current_visual_row = current_visual_row.saturating_add(status_visual_rows);
                }
            }

            if let (Some(prompt_line), Some(meas)) = (prompt_rendered, prompt_cursor) {
                if current_visual_row <= term_rows {
                    let prompt_start_row: usize = current_visual_row;
                    body_lines.push(prompt_line);
                    let cursor_row: usize = prompt_start_row
                        .saturating_add(meas.row_offset)
                        .min(term_rows);
                    let cursor_col: usize = meas.cursor_col.clamp(1, term_cols);
                    cursor_pos = Some((cursor_row, cursor_col));
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
