use unicode_width::UnicodeWidthChar;

/// Result of slicing a log buffer to fit the visible viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleLinesResult {
    pub visible_lines: Vec<String>,
    pub clamped_offset: usize,
}

/// Cursor measurement after rendering a prompt + user input line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorMeasurement {
    /// Total visual rows reserved for rendering the prompt and cursor.
    pub visual_rows: usize,
    /// Zero-based row offset from the prompt's start row where the cursor lands.
    pub row_offset: usize,
    /// One-based terminal column where the cursor lands.
    pub cursor_col: usize,
}

/// Strip ANSI CSI (`\x1b[...<final>`) and OSC (`\x1b]...(\x07|\x1b\\)`) escape sequences.
#[must_use]
pub fn strip_ansi(text: &str) -> String {
    let bytes: &[u8] = text.as_bytes();
    let len: usize = bytes.len();
    let mut out: String = String::with_capacity(len);
    let mut i: usize = 0;

    while i < len {
        if bytes[i] == 0x1b && i + 1 < len {
            match bytes[i + 1] {
                b'[' => {
                    i += 2;
                    while i < len {
                        let b: u8 = bytes[i];
                        i += 1;
                        if (0x40..=0x7e).contains(&b) {
                            break;
                        }
                    }
                    continue;
                }
                b']' => {
                    i += 2;
                    while i < len {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                }
                _ => {}
            }
        }

        // Copy valid UTF-8 scalar value starting at byte index `i`.
        if let Some(ch) = text[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    out
}

#[inline]
fn char_display_width(ch: char) -> usize {
    let code: u32 = u32::from(ch);
    if code < 0x20 || code == 0x7f {
        return 0;
    }
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

/// Compute visible terminal column width of `text` (ignoring ANSI escape codes).
#[must_use]
pub fn display_width(text: &str) -> usize {
    let plain: String = strip_ansi(text);
    plain.chars().map(char_display_width).sum()
}

/// Right-pad `text` with ASCII spaces until its display width reaches `target_width`.
#[must_use]
pub fn pad_display_width(text: &str, target_width: usize) -> String {
    let width: usize = display_width(text);
    let pad: usize = target_width.saturating_sub(width);
    if pad == 0 {
        return text.to_string();
    }
    let mut out: String = String::with_capacity(text.len() + pad);
    out.push_str(text);
    for _ in 0..pad {
        out.push(' ');
    }
    out
}

/// Count how many terminal visual rows `line` occupies when wrapped at `term_cols`.
///
/// Accounts for wide (CJK) characters wrapping to the next row when only 1 cell
/// remains at the end of a terminal row, and matches VT deferred-wrap semantics.
#[must_use]
pub fn count_visual_rows(line: &str, term_cols: usize) -> usize {
    let cols: usize = term_cols.max(1);
    let plain: String = strip_ansi(line);
    let mut rows: usize = 1;
    let mut col: usize = 0;

    for ch in plain.chars() {
        let w: usize = char_display_width(ch);
        if w == 0 {
            continue;
        }
        if col > 0 && col + w > cols {
            rows += 1;
            col = w;
        } else {
            col += w;
        }
    }

    rows
}

/// Measure visual row reservation and final cursor coordinates for a prompt line.
#[must_use]
pub fn measure_prompt_cursor(prompt_line: &str, term_cols: usize) -> CursorMeasurement {
    let cols: usize = term_cols.max(1);
    let plain: String = strip_ansi(prompt_line);
    let mut row_offset: usize = 0;
    let mut col: usize = 0;

    for ch in plain.chars() {
        let w: usize = char_display_width(ch);
        if w == 0 {
            continue;
        }
        if col > 0 && col + w > cols {
            row_offset += 1;
            col = w;
        } else {
            col += w;
        }
    }

    if col >= cols {
        row_offset += 1;
        col = 0;
    }

    CursorMeasurement {
        visual_rows: row_offset + 1,
        row_offset,
        cursor_col: col + 1,
    }
}

/// Truncate `line` (preserving ANSI CSI codes) so its display width is at most `max_width`,
/// appending `…` when truncation occurs.
#[must_use]
pub fn truncate_line_to_display_width(line: &str, max_width: usize) -> String {
    if max_width == 0 || display_width(line) <= max_width {
        return line.to_string();
    }

    let target_width: usize = max_width.saturating_sub(1);
    let bytes: &[u8] = line.as_bytes();
    let len: usize = bytes.len();
    let mut out: String = String::with_capacity(line.len().min(max_width.saturating_mul(4)));
    let mut width: usize = 0;
    let mut i: usize = 0;

    while i < len {
        if bytes[i] == 0x1b && i + 1 < len && bytes[i + 1] == b'[' {
            let start: usize = i;
            i += 2;
            while i < len {
                let b: u8 = bytes[i];
                i += 1;
                if (0x40..=0x7e).contains(&b) {
                    break;
                }
            }
            out.push_str(&line[start..i]);
            continue;
        }

        let Some(ch): Option<char> = line[i..].chars().next() else {
            break;
        };
        let ch_width: usize = char_display_width(ch);
        if width + ch_width > target_width {
            break;
        }
        out.push(ch);
        width += ch_width;
        i += ch.len_utf8();
    }

    out.push('…');
    out
}

/// Compute the maximum line scroll offset (from the bottom) so scrolling never
/// overshoots the oldest line at the top of the buffer.
#[must_use]
pub fn compute_max_scroll_offset(
    lines: &[String],
    available_rows: usize,
    term_cols: usize,
) -> usize {
    let total_rows: usize = lines
        .iter()
        .map(|line: &String| count_visual_rows(line, term_cols))
        .sum();
    if total_rows <= available_rows {
        return 0;
    }

    let mut used_from_top: usize = 0;
    let mut fit_count_from_top: usize = 0;
    for line in lines {
        let r: usize = count_visual_rows(line, term_cols);
        if used_from_top + r > available_rows && fit_count_from_top > 0 {
            break;
        }
        used_from_top += r;
        fit_count_from_top += 1;
    }

    lines.len().saturating_sub(fit_count_from_top)
}

/// Select the slice of lines visible in a viewport of `available_rows x term_cols`,
/// anchored from the bottom minus `current_offset`.
#[must_use]
pub fn select_visible_lines(
    lines: &[String],
    current_offset: usize,
    available_rows: usize,
    term_cols: usize,
) -> VisibleLinesResult {
    if lines.is_empty() || available_rows == 0 {
        return VisibleLinesResult {
            visible_lines: Vec::new(),
            clamped_offset: 0,
        };
    }

    let max_offset: usize = compute_max_scroll_offset(lines, available_rows, term_cols);
    let clamped_offset: usize = current_offset.min(max_offset);
    let end_idx: usize = lines.len().saturating_sub(clamped_offset);

    let mut selected: Vec<String> = Vec::new();
    let mut used_rows: usize = 0;

    for line in lines[..end_idx].iter().rev() {
        let rows_needed: usize = count_visual_rows(line, term_cols);
        if used_rows + rows_needed > available_rows {
            if selected.is_empty() {
                selected.push(truncate_line_to_display_width(
                    line,
                    available_rows * term_cols.max(1),
                ));
            }
            break;
        }
        selected.push(line.clone());
        used_rows += rows_needed;
        if used_rows >= available_rows {
            break;
        }
    }

    selected.reverse();
    VisibleLinesResult {
        visible_lines: selected,
        clamped_offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
