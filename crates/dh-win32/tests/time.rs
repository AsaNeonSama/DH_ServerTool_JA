use dh_win32::{get_local_time, LocalTimeParts};

#[test]
fn get_local_time_returns_valid_calendar_and_clock_bounds() {
    let now: LocalTimeParts = get_local_time();
    assert!(now.year >= 2024, "unexpected year: {}", now.year);
    assert!(
        (1..=12).contains(&now.month),
        "invalid month: {}",
        now.month
    );
    assert!((1..=31).contains(&now.day), "invalid day: {}", now.day);
    assert!(now.hour <= 23, "invalid hour: {}", now.hour);
    assert!(now.minute <= 59, "invalid minute: {}", now.minute);
    assert!(now.second <= 59, "invalid second: {}", now.second);
    assert!(
        now.millisecond <= 999,
        "invalid millisecond: {}",
        now.millisecond
    );
}
