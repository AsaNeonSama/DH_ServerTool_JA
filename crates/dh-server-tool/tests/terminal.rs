use dh_server_tool::terminal::display::{
    count_visual_rows, display_width, measure_prompt_cursor, pad_display_width,
    select_visible_lines, strip_ansi, truncate_line_to_display_width, CursorMeasurement,
};
use dh_server_tool::terminal::post_match::build_countdown_text;
use dh_server_tool::terminal::tab_renderer::{
    build_tab_bar, render_frame_buffer, FrameRenderInput, TerminalTabId,
};
use dh_server_tool::web_console::WebConsoleDisplayInfo;

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
fn strip_ansi_removes_sgr_and_osc() {
    let input = "\x1b[96m * ソフト作者:\x1b[97m アサネオン\x1b[0m";
    assert_eq!(strip_ansi(input), " * ソフト作者: アサネオン");

    let osc = "\x1b]0;DH_Server_Tool_JA\x07Hello";
    assert_eq!(strip_ansi(osc), "Hello");
}

#[test]
fn display_width_handles_ascii_cjk_and_ansi() {
    assert_eq!(display_width("abc"), 3);
    assert_eq!(display_width("山頂"), 4);
    assert_eq!(display_width("\x1b[92m1:役職ランダム\x1b[0m"), 14);
    assert_eq!(display_width("─│▲✓…"), 5);
}

#[test]
fn pad_display_width_aligns_cjk_cells() {
    let padded = pad_display_width("1:役職ランダム", 20);
    assert_eq!(display_width(&padded), 20);
    assert!(padded.ends_with("      "));
    assert_eq!(pad_display_width("1:役職ランダム", 10), "1:役職ランダム");
}

#[test]
fn count_visual_rows_wraps_cjk_at_odd_boundary() {
    assert_eq!(count_visual_rows("", 10), 1);
    assert_eq!(count_visual_rows("1234567890", 10), 1);
    assert_eq!(count_visual_rows("12345678901", 10), 2);
    assert_eq!(count_visual_rows("123456789あ", 10), 2);
    assert_eq!(count_visual_rows("あいうえお", 10), 1);
    assert_eq!(count_visual_rows("あいうえおか", 10), 2);
    assert_eq!(count_visual_rows("あいう", 5), 2);
}

#[test]
fn measure_prompt_cursor_accounts_for_exact_and_cjk_wraps() {
    let m1 = measure_prompt_cursor("abc", 10);
    assert_eq!(
        m1,
        CursorMeasurement {
            visual_rows: 1,
            row_offset: 0,
            cursor_col: 4,
        }
    );

    let m2 = measure_prompt_cursor("0123456789", 10);
    assert_eq!(
        m2,
        CursorMeasurement {
            visual_rows: 2,
            row_offset: 1,
            cursor_col: 1,
        }
    );

    let m3 = measure_prompt_cursor("123456789山", 10);
    assert_eq!(
        m3,
        CursorMeasurement {
            visual_rows: 2,
            row_offset: 1,
            cursor_col: 3,
        }
    );
}

#[test]
fn truncate_line_preserves_ansi_and_respects_cjk_width() {
    let line = "\x1b[92mあいうえお\x1b[0m";
    let truncated = truncate_line_to_display_width(line, 6);
    assert!(truncated.starts_with("\x1b[92m"));
    assert!(truncated.ends_with('…'));
    assert!(display_width(&truncated) <= 6);
    assert_eq!(strip_ansi(&truncated), "あい…");

    let exact_one = truncate_line_to_display_width("あいう", 1);
    assert_eq!(exact_one, "…");
    assert_eq!(display_width(&exact_one), 1);
}

#[test]
fn select_visible_lines_clamps_scroll_and_handles_wrapping() {
    let lines: Vec<String> = (1..=10).map(|i| format!("line {i}")).collect();
    let bottom = select_visible_lines(&lines, 0, 3, 20);
    assert_eq!(bottom.clamped_offset, 0);
    assert_eq!(bottom.visible_lines, vec!["line 8", "line 9", "line 10"]);

    let scrolled = select_visible_lines(&lines, 100, 3, 20);
    assert_eq!(scrolled.clamped_offset, 7);
    assert_eq!(scrolled.visible_lines, vec!["line 1", "line 2", "line 3"]);

    let huge = vec!["あ".repeat(30)];
    let res = select_visible_lines(&huge, 0, 2, 10);
    assert_eq!(res.visible_lines.len(), 1);
    assert!(display_width(&res.visible_lines[0]) <= 20);
    assert!(res.visible_lines[0].ends_with('…'));
}

#[test]
fn countdown_text_matches_japanese_format() {
    let rendered = strip_ansi(&build_countdown_text(10));
    assert_eq!(
        rendered,
        " * 自動再起動まで残り 10 秒... (R: マップ・MOD再設定 / Enter: 今すぐ再起動)"
    );
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
