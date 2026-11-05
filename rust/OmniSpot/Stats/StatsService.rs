use super::session::{DailyBucket, SessionStats};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Serialize, Deserialize, Default)]
struct StatsDocument {
    buckets: Vec<DailyBucket>,
}
pub struct StatsService {
    enabled: bool,
    path: PathBuf,
    session: Arc<Mutex<SessionStats>>,
}
impl StatsService {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            enabled: false,
            path: path.into(),
            session: Arc::new(Mutex::new(SessionStats::default())),
        }
    }
    pub fn start(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.load();
        }
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.flush();
        }
        self.enabled = enabled;
    }
    pub fn record_switch(
        &self,
        process: &str,
        duration_ms: i32,
        choices: i32,
        dynamic: bool,
        key: crate::configuration::Key,
    ) {
        if self.enabled {
            if let Ok(mut session) = self.session.lock() {
                session.record_switch(
                    process,
                    duration_ms,
                    super::EfficiencyCalculator::saved_ms(choices, duration_ms),
                    dynamic,
                    Some(duration_ms),
                    key,
                );
            }
        }
    }
    pub fn record_peek(&self, process: &str, duration_ms: i32, dynamic: bool) {
        if self.enabled {
            if let Ok(mut session) = self.session.lock() {
                session.record_peek(process, duration_ms, dynamic);
            }
        }
    }
    pub fn record_alt_tab(&self, count: i32) {
        if self.enabled {
            if let Ok(mut session) = self.session.lock() {
                session.record_alt_tab(count);
            }
        }
    }
    pub fn snapshots(&self) -> Vec<DailyBucket> {
        self.session
            .lock()
            .map(|session| session.snapshots())
            .unwrap_or_default()
    }

    pub fn metrics(&self) -> super::StatsMetrics {
        let snapshots = self.snapshots();
        let Some(today) = snapshots.last() else {
            return super::StatsCalculator::compute(&[], &DailyBucket::today());
        };
        super::StatsCalculator::compute(&snapshots[..snapshots.len() - 1], today)
    }
    pub fn flush(&self) {
        if !self.enabled {
            return;
        }
        let buckets = self.snapshots();
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(&StatsDocument { buckets }) {
            let _ = fs::write(&self.path, text);
        }
    }
    fn load(&mut self) {
        if let Ok(text) = fs::read_to_string(&self.path) {
            if let Ok(document) = serde_json::from_str::<StatsDocument>(&text) {
                if let Ok(mut session) = self.session.lock() {
                    session.load_from_buckets(&document.buckets);
                }
            }
        }
    }
}
