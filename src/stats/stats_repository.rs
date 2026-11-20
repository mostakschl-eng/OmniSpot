use std::sync::{Arc, Mutex};
use super::storage::{DailyBucketDocument, StatsDatabase};

pub(crate) struct StatsRepository {
    db: Arc<Mutex<StatsDatabase>>,
}

impl StatsRepository {
    pub(crate) fn new(db: Arc<Mutex<StatsDatabase>>) -> Self {
        Self { db }
    }

    /// Returns all historical daily buckets strictly before `today`, ordered descending by date.
    pub(crate) fn get_all_historic_buckets(&self, today: Option<&str>) -> Vec<DailyBucketDocument> {
        let today_str = today.map(|s| s.to_string()).unwrap_or_else(|| {
            crate::stats::session_stats::system_today()
        });

        let Ok(db) = self.db.lock() else {
            return Vec::new();
        };

        let mut results: Vec<DailyBucketDocument> = db
            .daily_buckets
            .values()
            .filter_map(|val| serde_json::from_value::<DailyBucketDocument>(val.clone()).ok())
            .filter(|bucket| bucket.date < today_str)
            .collect();

        results.sort_by(|a, b| b.date.cmp(&a.date));
        results
    }
}

#[cfg(test)]
#[path = "../tests/stats/stats_repository_tests.rs"]
mod tests;
