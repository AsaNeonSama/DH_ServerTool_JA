use dh_win32::console::{get_clipboard_text, init_console, set_clipboard_text};

#[test]
fn clipboard_roundtrip_ascii_and_japanese() {
    // Save previous clipboard text (best-effort) so running tests doesn't clobber user clipboard.
    let prev = get_clipboard_text().ok();

    let sample = "http://127.0.0.1:7891/dh-console/auth#日本語テスト_Abc123-_";
    set_clipboard_text(sample).expect("set_clipboard_text");
    let read_back = get_clipboard_text().expect("get_clipboard_text");
    assert_eq!(read_back, sample);

    set_clipboard_text("").expect("set_clipboard_text empty");
    let empty_back = get_clipboard_text().expect("get_clipboard_text empty");
    assert_eq!(empty_back, "");

    if let Some(old) = prev {
        let _ = set_clipboard_text(&old);
    }
}

#[test]
fn init_console_does_not_panic_when_redirected() {
    init_console("DH_Tool_JA テスト");
}
