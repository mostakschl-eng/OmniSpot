const ALT_TAB_BASE_OVERHEAD: i32 = 100;
const ALT_TAB_SCAN_FACTOR: i32 = 140;

/// T_at = Base + (n * Scan)
/// 380ms for 2 windows but 1500ms for 10 windows
pub(crate) fn alt_tab_time_ms(window_count: i32) -> i32 {
    ALT_TAB_BASE_OVERHEAD + (window_count * ALT_TAB_SCAN_FACTOR)
}

/// Milliseconds saved by using OmniSpot instead of Alt+Tab. Never negative.
pub(crate) fn saved_ms(window_count: i32, actual_duration_ms: i32) -> i32 {
    (alt_tab_time_ms(window_count) - actual_duration_ms).max(0)
}

#[cfg(test)]
#[path = "../tests/stats/efficiency_calculator_tests.rs"]
mod tests;
