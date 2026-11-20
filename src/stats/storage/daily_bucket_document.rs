use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

pub const COLLECTION_NAME: &str = "daily_buckets";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FastestSwitchRecord {
    pub duration_ms: i32,
    pub app_name: String,
    pub letter: String,
    pub date: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsageStats {
    pub switches: i32,
    pub peeks: i32,
    pub total_peek_time_ms: i32,
    pub total_switch_time_ms: i32,
}

impl AppUsageStats {
    pub fn new(switches: i32, peeks: i32, total_peek_time_ms: i32, total_switch_time_ms: i32) -> Self {
        Self {
            switches,
            peeks,
            total_peek_time_ms,
            total_switch_time_ms,
        }
    }

    pub fn new_switch(duration_ms: i32) -> Self {
        Self {
            switches: 1,
            total_switch_time_ms: duration_ms,
            ..Default::default()
        }
    }

    pub fn new_peek(duration_ms: i32) -> Self {
        Self {
            peeks: 1,
            total_peek_time_ms: duration_ms,
            ..Default::default()
        }
    }

    pub fn add_switch(&mut self, duration_ms: i32) {
        self.switches += 1;
        self.total_switch_time_ms += duration_ms;
    }

    pub fn add_peek(&mut self, duration_ms: i32) {
        self.peeks += 1;
        self.total_peek_time_ms += duration_ms;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyBucketDocument {
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub total_switches: i32,
    #[serde(default)]
    pub total_time_saved_ms: i32,
    #[serde(default)]
    pub total_peeks: i32,
    #[serde(default)]
    pub static_app_usage: BTreeMap<String, AppUsageStats>,
    #[serde(default)]
    pub dynamic_app_usage: BTreeMap<String, AppUsageStats>,
    #[serde(default)]
    pub alt_tab_switches: i32,
    #[serde(default)]
    pub alt_tab_keystrokes: i32,
    #[serde(default)]
    pub fastest_switch: Option<FastestSwitchRecord>,
}

impl DailyBucketDocument {
    pub fn new(date: impl Into<String>) -> Self {
        Self {
            date: date.into(),
            ..Default::default()
        }
    }

    pub fn record_switch(
        &mut self,
        process_name: &str,
        duration_ms: i32,
        saved_ms: i32,
        is_dynamic: bool,
    ) {
        self.total_switches += 1;
        self.total_time_saved_ms += saved_ms;

        let bucket = if is_dynamic {
            &mut self.dynamic_app_usage
        } else {
            &mut self.static_app_usage
        };

        if let Some(existing) = bucket.get_mut(process_name) {
            existing.add_switch(duration_ms);
        } else {
            bucket.insert(
                process_name.to_string(),
                AppUsageStats::new_switch(duration_ms),
            );
        }
    }

    pub fn record_peek(&mut self, process_name: &str, duration_ms: i32, is_dynamic: bool) {
        self.total_peeks += 1;

        let bucket = if is_dynamic {
            &mut self.dynamic_app_usage
        } else {
            &mut self.static_app_usage
        };

        if let Some(existing) = bucket.get_mut(process_name) {
            existing.add_peek(duration_ms);
        } else {
            bucket.insert(
                process_name.to_string(),
                AppUsageStats::new_peek(duration_ms),
            );
        }
    }

    pub fn record_fastest_switch(
        &mut self,
        process_name: &str,
        fastest_duration_ms: Option<i32>,
        letter: &str,
        date: &str,
    ) {
        if let Some(duration) = fastest_duration_ms {
            if duration > 0
                && (self.fastest_switch.is_none()
                    || duration < self.fastest_switch.as_ref().unwrap().duration_ms)
            {
                self.fastest_switch = Some(FastestSwitchRecord {
                    duration_ms: duration,
                    app_name: process_name.to_string(),
                    letter: letter.to_string(),
                    date: date.to_string(),
                });
            }
        }
    }

    pub fn record_alt_tab(&mut self, tab_count: i32) {
        self.alt_tab_switches += 1;
        self.alt_tab_keystrokes += tab_count;
    }
}
