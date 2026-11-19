use super::*;

#[test]
fn alt_tab_time_ms_returns_expected_value() {
    let cases = [(0, 100), (1, 240), (3, 520), (7, 1080)];
    for (window_count, expected_ms) in cases {
        assert_eq!(alt_tab_time_ms(window_count), expected_ms);
    }
}

#[test]
fn saved_ms_returns_alt_tab_minus_omni_spot() {
    let cases = [
        (1, 100, 240 - 100),
        (1, 200, 240 - 200),
        (3, 50, 520 - 50),
        (7, 80, 1080 - 80),
    ];
    for (window_count, actual_duration_ms, expected_saved_ms) in cases {
        assert_eq!(
            saved_ms(window_count, actual_duration_ms),
            expected_saved_ms
        );
    }
}

#[test]
fn saved_ms_never_returns_negative() {
    let result = saved_ms(0, 100);
    assert!(result >= 0);
}

#[test]
fn saved_ms_increases_with_more_choices() {
    let saved_for_1 = saved_ms(1, 100);
    let saved_for_10 = saved_ms(10, 100);
    assert!(saved_for_10 > saved_for_1);
}
