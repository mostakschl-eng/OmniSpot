pub struct EfficiencyCalculator;
impl EfficiencyCalculator {
    pub fn alt_tab_time_ms(window_count: i32) -> i32 {
        100 + window_count * 140
    }
    pub fn saved_ms(window_count: i32, actual_duration_ms: i32) -> i32 {
        (Self::alt_tab_time_ms(window_count) - actual_duration_ms).max(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_time_never_negative() {
        assert_eq!(EfficiencyCalculator::saved_ms(2, 1000), 0);
        assert_eq!(EfficiencyCalculator::saved_ms(2, 100), 280);
    }
}
