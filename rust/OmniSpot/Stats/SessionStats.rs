use crate::configuration::Key;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppUsageStats {
    pub switches: i32,
    pub peeks: i32,
    pub total_peek_time_ms: i32,
    pub total_switch_time_ms: i32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastestSwitchRecord {
    pub duration_ms: i32,
    pub app_name: String,
    pub letter: Key,
    pub timestamp_ms: u128,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyBucket {
    pub date: String,
    pub total_switches: i32,
    pub total_time_saved_ms: i32,
    pub total_peeks: i32,
    pub static_app_usage: HashMap<String, AppUsageStats>,
    pub dynamic_app_usage: HashMap<String, AppUsageStats>,
    pub alt_tab_switches: i32,
    pub alt_tab_keystrokes: i32,
    pub fastest_switch: Option<FastestSwitchRecord>,
}
impl DailyBucket {
    pub fn today() -> Self {
        Self {
            date: today(),
            total_switches: 0,
            total_time_saved_ms: 0,
            total_peeks: 0,
            static_app_usage: HashMap::new(),
            dynamic_app_usage: HashMap::new(),
            alt_tab_switches: 0,
            alt_tab_keystrokes: 0,
            fastest_switch: None,
        }
    }
    pub fn record_switch(
        &mut self,
        process: &str,
        duration_ms: i32,
        saved_ms: i32,
        dynamic: bool,
        fastest_ms: Option<i32>,
        key: Key,
    ) {
        self.total_switches += 1;
        self.total_time_saved_ms += saved_ms;
        let bucket = if dynamic {
            &mut self.dynamic_app_usage
        } else {
            &mut self.static_app_usage
        };
        let stats = bucket.entry(process.to_string()).or_default();
        stats.switches += 1;
        stats.total_switch_time_ms += duration_ms;
        if let Some(duration) = fastest_ms
            .filter(|duration| *duration > 0)
            .filter(|duration| {
                self.fastest_switch
                    .as_ref()
                    .map(|record| *duration < record.duration_ms)
                    .unwrap_or(true)
            })
        {
            self.fastest_switch = Some(FastestSwitchRecord {
                duration_ms: duration,
                app_name: process.to_string(),
                letter: key,
                timestamp_ms: now_ms(),
            });
        }
    }
    pub fn record_peek(&mut self, process: &str, duration_ms: i32, dynamic: bool) {
        self.total_peeks += 1;
        let bucket = if dynamic {
            &mut self.dynamic_app_usage
        } else {
            &mut self.static_app_usage
        };
        let stats = bucket.entry(process.to_string()).or_default();
        stats.peeks += 1;
        stats.total_peek_time_ms += duration_ms;
    }
    pub fn record_alt_tab(&mut self, count: i32) {
        self.alt_tab_switches += 1;
        self.alt_tab_keystrokes += count;
    }
}

pub struct SessionStats {
    history: Vec<DailyBucket>,
    current: DailyBucket,
    recent: Vec<String>,
}
impl Default for SessionStats {
    fn default() -> Self {
        Self {
            history: Vec::new(),
            current: DailyBucket::today(),
            recent: Vec::new(),
        }
    }
}
impl SessionStats {
    pub fn record_switch(
        &mut self,
        process: &str,
        duration_ms: i32,
        saved_ms: i32,
        dynamic: bool,
        fastest_ms: Option<i32>,
        key: Key,
    ) {
        self.rollover();
        self.current
            .record_switch(process, duration_ms, saved_ms, dynamic, fastest_ms, key);
        self.recent
            .retain(|name| !name.eq_ignore_ascii_case(process));
        self.recent.insert(0, process.to_string());
        self.recent.truncate(20);
    }
    pub fn record_peek(&mut self, process: &str, duration_ms: i32, dynamic: bool) {
        self.rollover();
        self.current.record_peek(process, duration_ms, dynamic);
    }
    pub fn record_alt_tab(&mut self, count: i32) {
        self.rollover();
        self.current.record_alt_tab(count);
    }
    pub fn snapshots(&self) -> Vec<DailyBucket> {
        self.history
            .iter()
            .cloned()
            .chain(std::iter::once(self.current.clone()))
            .collect()
    }
    pub fn load_from(&mut self, bucket: DailyBucket) {
        self.current = bucket;
        self.history.clear();
    }

    pub fn load_from_buckets(&mut self, buckets: &[DailyBucket]) {
        let mut buckets = buckets.to_vec();
        buckets.sort_by(|left, right| left.date.cmp(&right.date));
        let Some(current) = buckets.pop() else {
            return;
        };
        self.current = current;
        self.history = buckets;
    }
    pub fn recent_process_names(&self) -> Vec<String> {
        self.recent.clone()
    }
    fn rollover(&mut self) {
        if self.current.date != today() {
            self.history.push(self.current.clone());
            self.current = DailyBucket::today();
        }
    }
}
fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
fn today() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("day-{}", seconds / 86_400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_all_historical_buckets_across_rollover() {
        let mut stats = SessionStats::default();
        stats.current.date = "day-10".to_string();
        stats.current.total_switches = 2;
        stats.rollover();
        stats.current.total_switches = 3;
        stats.current.date = "day-11".to_string();
        stats.rollover();
        assert_eq!(stats.snapshots().len(), 3);
        assert_eq!(stats.snapshots()[0].total_switches, 2);
    }

    #[test]
    fn loads_all_buckets_in_date_order() {
        let mut stats = SessionStats::default();
        let mut first = DailyBucket::today();
        first.date = "day-1".to_string();
        let mut second = DailyBucket::today();
        second.date = "day-2".to_string();
        stats.load_from_buckets(&[second, first]);
        assert_eq!(stats.snapshots()[0].date, "day-1");
        assert_eq!(stats.snapshots()[1].date, "day-2");
    }
}
