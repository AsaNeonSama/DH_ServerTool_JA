use dh_server_tool::format::{format_seconds, sanitize_log_text};

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
