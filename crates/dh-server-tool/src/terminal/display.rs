use unicode_width::UnicodeWidthChar;

pub use crate::structs::terminal::{CursorMeasurement, VisibleLinesResult};

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
        if let Some(ch) = text.get(i..).and_then(|rest: &str| rest.chars().next()) {
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
    let mut out: String = String::with_capacity(text.len().saturating_add(pad));
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
        if col > 0 && col.saturating_add(w) > cols {
            rows = rows.saturating_add(1);
            col = w;
        } else {
            col = col.saturating_add(w);
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
        if col > 0 && col.saturating_add(w) > cols {
            row_offset = row_offset.saturating_add(1);
            col = w;
        } else {
            col = col.saturating_add(w);
        }
    }

    if col >= cols {
        row_offset = row_offset.saturating_add(1);
        col = 0;
    }

    CursorMeasurement {
        visual_rows: row_offset.saturating_add(1),
        row_offset,
        cursor_col: col.saturating_add(1),
    }
}

/// Truncate `line` (preserving ANSI CSI codes and skipping OSC sequences) so its display width
/// is at most `max_width`, appending `…` when truncation occurs.
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
        if bytes[i] == 0x1b && i + 1 < len {
            match bytes[i + 1] {
                b'[' => {
                    let start: usize = i;
                    i += 2;
                    while i < len {
                        let b: u8 = bytes[i];
                        i += 1;
                        if (0x40..=0x7e).contains(&b) {
                            break;
                        }
                    }
                    if let Some(seq) = line.get(start..i) {
                        out.push_str(seq);
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

        let Some(ch): Option<char> = line.get(i..).and_then(|rest: &str| rest.chars().next())
        else {
            break;
        };
        let ch_width: usize = char_display_width(ch);
        if width.saturating_add(ch_width) > target_width {
            break;
        }
        out.push(ch);
        width = width.saturating_add(ch_width);
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
    let total_rows: usize = lines.iter().fold(0usize, |acc: usize, line: &String| {
        acc.saturating_add(count_visual_rows(line, term_cols))
    });
    if total_rows <= available_rows {
        return 0;
    }

    let mut used_from_top: usize = 0;
    let mut fit_count_from_top: usize = 0;
    for line in lines {
        let r: usize = count_visual_rows(line, term_cols);
        if used_from_top.saturating_add(r) > available_rows && fit_count_from_top > 0 {
            break;
        }
        used_from_top = used_from_top.saturating_add(r);
        fit_count_from_top = fit_count_from_top.saturating_add(1);
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
        if used_rows.saturating_add(rows_needed) > available_rows {
            if selected.is_empty() {
                selected.push(truncate_line_to_display_width(
                    line,
                    available_rows.saturating_mul(term_cols.max(1)),
                ));
            }
            break;
        }
        selected.push(line.clone());
        used_rows = used_rows.saturating_add(rows_needed);
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
