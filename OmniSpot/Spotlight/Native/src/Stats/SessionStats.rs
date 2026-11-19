use std::collections::HashMap;
use std::sync::Mutex;
pub(crate) use super::storage::DailyBucketDocument;
#[cfg(test)]
pub(crate) use super::storage::AppUsageStats;
use super::ISessionStats;

pub(crate) fn format_date_from_epoch_days(days: i64) -> String {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

pub(crate) fn system_today() -> String {
    #[cfg(windows)]
    {
        let local = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        return format!("{:04}-{:02}-{:02}", local.wYear, local.wMonth, local.wDay);
    }
    #[cfg(not(windows))]
    {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let days = (duration.as_secs() / 86400) as i64;
    format_date_from_epoch_days(days)
    }
}

#[derive(Default)]
struct SessionStatsData {
    rollover_snapshot: Option<DailyBucketDocument>,
    current_snapshot: DailyBucketDocument,
    recent_process_names: Vec<String>,
    today_override: Option<String>,
}

impl SessionStatsData {
    fn new() -> Self {
        let today = system_today();
        Self {
            rollover_snapshot: None,
            current_snapshot: DailyBucketDocument::new(&today),
            recent_process_names: Vec::new(),
            today_override: None,
        }
    }

    fn today(&self) -> String {
        self.today_override.clone().unwrap_or_else(system_today)
    }

    fn check_rollover(&mut self) {
        let today = self.today();
        if today == self.current_snapshot.date {
            return;
        }

        if self.has_any_data() {
            self.rollover_snapshot = Some(self.current_snapshot.clone());
        }

        self.current_snapshot = DailyBucketDocument::new(&today);
    }

    fn has_any_data(&self) -> bool {
        self.current_snapshot.total_switches > 0
            || self.current_snapshot.total_peeks > 0
            || self.current_snapshot.alt_tab_switches > 0
    }
}

pub(crate) struct SessionStats {
    data: Mutex<SessionStatsData>,
}

impl Default for SessionStats {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStats {
    pub(crate) fn new() -> Self {
        Self {
            data: Mutex::new(SessionStatsData::new()),
        }
    }

    pub(crate) fn record_switch(
        &self,
        process_name: &str,
        _previous_process_name: Option<&str>,
        duration_ms: i32,
        saved_ms: i32,
        is_dynamic: bool,
        fastest_duration_ms: Option<i32>,
        trigger_key: &str,
    ) {
        let mut d = self.data.lock().unwrap();
        d.check_rollover();
        d.current_snapshot
            .record_switch(process_name, duration_ms, saved_ms, is_dynamic);
        let today = d.today();
        d.current_snapshot.record_fastest_switch(
            process_name,
            fastest_duration_ms,
            trigger_key,
            &today,
        );

        if let Some(pos) = d
            .recent_process_names
            .iter()
            .position(|p| p.eq_ignore_ascii_case(process_name))
        {
            d.recent_process_names.remove(pos);
        }
        d.recent_process_names.insert(0, process_name.to_string());
        if d.recent_process_names.len() > 20 {
            d.recent_process_names.pop();
        }
    }

    pub(crate) fn record_peek(&self, process_name: &str, duration_ms: i32, is_dynamic: bool) {
        let mut d = self.data.lock().unwrap();
        d.check_rollover();
        d.current_snapshot
            .record_peek(process_name, duration_ms, is_dynamic);
    }

    pub(crate) fn record_alt_tab(&self, tab_count: i32) {
        let mut d = self.data.lock().unwrap();
        d.check_rollover();
        d.current_snapshot.record_alt_tab(tab_count);
    }

    pub(crate) fn load_from(&self, doc: &DailyBucketDocument) {
        let mut d = self.data.lock().unwrap();
        d.current_snapshot = doc.clone();
        d.rollover_snapshot = None;
    }

    pub(crate) fn clear(&self) {
        let mut d = self.data.lock().unwrap();
        let today = d.today();
        d.current_snapshot = DailyBucketDocument::new(&today);
        d.rollover_snapshot = None;
    }

    pub(crate) fn clear_rollover(&self) {
        let mut d = self.data.lock().unwrap();
        d.rollover_snapshot = None;
    }

    pub(crate) fn get_snapshots(&self) -> Vec<DailyBucketDocument> {
        let mut d = self.data.lock().unwrap();
        d.check_rollover();
        let today = d.current_snapshot.clone();
        match &d.rollover_snapshot {
            Some(prev) => vec![prev.clone(), today],
            None => vec![today],
        }
    }

    pub(crate) fn simulate_last_recorded_date(&self, date: impl Into<String>) {
        let mut d = self.data.lock().unwrap();
        d.current_snapshot.date = date.into();
    }

    #[allow(dead_code)]
    pub(crate) fn set_today_override(&self, date: Option<String>) {
        let mut d = self.data.lock().unwrap();
        d.today_override = date;
    }

    pub(crate) fn get_recent_process_names(&self) -> Vec<String> {
        let d = self.data.lock().unwrap();
        d.recent_process_names.clone()
    }

    pub(crate) fn get_usage_counts(&self) -> HashMap<String, i32> {
        let mut d = self.data.lock().unwrap();
        d.check_rollover();
        let mut usage: HashMap<String, i32> = HashMap::new();

        let buckets = [d.rollover_snapshot.clone(), Some(d.current_snapshot.clone())];
        for bucket in buckets.into_iter().flatten() {
            for (key, stats) in bucket.static_app_usage {
                *usage.entry(key.to_ascii_lowercase()).or_insert(0) += stats.switches;
            }
            for (key, stats) in bucket.dynamic_app_usage {
                *usage.entry(key.to_ascii_lowercase()).or_insert(0) += stats.switches;
            }
        }

        usage
    }
}

impl ISessionStats for SessionStats {
    fn record_switch(
        &mut self,
        process_name: &str,
        previous_process_name: Option<&str>,
        duration_ms: i32,
        saved_ms: i32,
        is_dynamic: bool,
        fastest_duration_ms: Option<i32>,
        trigger_key: &str,
    ) {
        SessionStats::record_switch(
            self,
            process_name,
            previous_process_name,
            duration_ms,
            saved_ms,
            is_dynamic,
            fastest_duration_ms,
            trigger_key,
        );
    }

    fn record_peek(&mut self, process_name: &str, duration_ms: i32, is_dynamic: bool) {
        SessionStats::record_peek(self, process_name, duration_ms, is_dynamic);
    }

    fn record_alt_tab(&mut self, tab_count: i32) {
        SessionStats::record_alt_tab(self, tab_count);
    }
}

impl ISessionStats for std::sync::Arc<SessionStats> {
    fn record_switch(
        &mut self,
        process_name: &str,
        previous_process_name: Option<&str>,
        duration_ms: i32,
        saved_ms: i32,
        is_dynamic: bool,
        fastest_duration_ms: Option<i32>,
        trigger_key: &str,
    ) {
        self.as_ref().record_switch(
            process_name,
            previous_process_name,
            duration_ms,
            saved_ms,
            is_dynamic,
            fastest_duration_ms,
            trigger_key,
        );
    }

    fn record_peek(&mut self, process_name: &str, duration_ms: i32, is_dynamic: bool) {
        self.as_ref().record_peek(process_name, duration_ms, is_dynamic);
    }

    fn record_alt_tab(&mut self, tab_count: i32) {
        self.as_ref().record_alt_tab(tab_count);
    }
}

#[cfg(test)]
#[path = "../tests/Stats/SessionStatsTests.rs"]
mod tests;
