use super::session::DailyBucket;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub(super) struct StatsDocument {
    #[serde(default = "legacy_schema_version")]
    pub(super) schema_version: u32,
    pub buckets: Vec<DailyBucket>,
}

const fn legacy_schema_version() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_stats_json_migrates_and_drops_the_default_date_bucket() {
        let json = r#"{"buckets":[{"date":"0001-01-01","total_switches":0,"total_time_saved_ms":0,"total_peeks":0,"static_app_usage":{},"dynamic_app_usage":{},"alt_tab_switches":0,"alt_tab_keystrokes":0,"fastest_switch":null}]}"#;
        let mut document: StatsDocument = serde_json::from_str(json).unwrap();
        assert_eq!(document.schema_version, 1);
        crate::stats::migrations::run_pending(&mut document).unwrap();
        assert_eq!(document.schema_version, 2);
        assert!(document.buckets.is_empty());
    }
}
