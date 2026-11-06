use super::{
    session::{DailyBucket, SessionStats},
    storage::StatsDocument,
};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub struct StatsService {
    enabled: bool,
    storage_healthy: bool,
    path: PathBuf,
    session: Arc<Mutex<SessionStats>>,
}
impl StatsService {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            enabled: false,
            storage_healthy: true,
            path: path.into(),
            session: Arc::new(Mutex::new(SessionStats::default())),
        }
    }
    pub fn start(&mut self, enabled: bool) {
        if enabled {
            self.storage_healthy = true;
            self.load();
        }
        self.enabled = enabled && self.storage_healthy;
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.flush();
            self.enabled = false;
            return;
        }
        if !self.enabled {
            self.storage_healthy = true;
            self.load();
        }
        self.enabled = self.storage_healthy;
    }
    pub fn record_switch(
        &self,
        process: &str,
        duration_ms: i32,
        fastest_ms: Option<i32>,
        choices: i32,
        dynamic: bool,
        key: crate::configuration::Key,
    ) {
        if self.enabled
            && let Ok(mut session) = self.session.lock()
        {
            session.record_switch(
                process_name(process),
                duration_ms,
                super::EfficiencyCalculator::saved_ms(choices, duration_ms),
                dynamic,
                fastest_ms,
                key,
            );
        }
    }
    pub fn record_peek(&self, process: &str, duration_ms: i32, dynamic: bool) {
        if self.enabled
            && let Ok(mut session) = self.session.lock()
        {
            session.record_peek(process_name(process), duration_ms, dynamic);
        }
    }
    pub fn record_alt_tab(&self, count: i32) {
        if self.enabled
            && let Ok(mut session) = self.session.lock()
        {
            session.record_alt_tab(count);
        }
    }
    pub fn snapshots(&self) -> Vec<DailyBucket> {
        self.session
            .lock()
            .map(|session| session.snapshots())
            .unwrap_or_default()
    }

    pub fn recent_process_names(&self) -> Vec<String> {
        self.session
            .lock()
            .map(|session| session.recent_process_names())
            .unwrap_or_default()
    }

    pub fn metrics(
        &self,
        configured_static_apps: &[(String, String, String)],
    ) -> super::StatsMetrics {
        let snapshots = self.snapshots();
        let Some(today) = snapshots.last() else {
            return super::StatsCalculator::compute(
                &[],
                &DailyBucket::today(),
                configured_static_apps,
            );
        };
        super::StatsCalculator::compute(
            &snapshots[..snapshots.len() - 1],
            today,
            configured_static_apps,
        )
    }
    pub fn flush(&self) {
        if !self.enabled || !self.storage_healthy {
            return;
        }
        let buckets = self.snapshots();
        if let Ok(text) = serde_json::to_string_pretty(&StatsDocument {
            schema_version: super::migrations::CURRENT_VERSION,
            buckets,
        }) && let Err(error) = crate::atomic_file::write(&self.path, text.as_bytes())
        {
            crate::logging::info(format!("Failed to save statistics: {error}"));
        }
    }
    pub fn reset(&mut self) -> Result<(), String> {
        match fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        if let Ok(mut session) = self.session.lock() {
            session.clear();
        }
        if self.enabled {
            self.flush();
        }
        Ok(())
    }
    fn load(&mut self) {
        if !self.path.exists() {
            if let Some(data_dir) = self.path.parent() {
                match crate::legacy_import::profile_bundle(data_dir) {
                    Ok(Some(bundle)) => {
                        if let Ok(mut session) = self.session.lock() {
                            session.load_from_buckets(&bundle.stats.buckets);
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        self.storage_healthy = false;
                        crate::logging::info(format!("Failed to load legacy stats: {error}"))
                    }
                }
            }
            return;
        }
        match fs::read_to_string(&self.path) {
            Ok(text) => match serde_json::from_str::<StatsDocument>(&text) {
                Ok(mut document) => match super::migrations::run_pending(&mut document) {
                    Ok(true) => {
                        if let Ok(migrated) = serde_json::to_vec_pretty(&document)
                            && let Err(error) = crate::atomic_file::write(&self.path, &migrated)
                        {
                            crate::logging::info(format!("Failed to save migrated stats: {error}"));
                        }
                        if let Ok(mut session) = self.session.lock() {
                            session.load_from_buckets(&document.buckets);
                        }
                    }
                    Ok(false) => {
                        if let Ok(mut session) = self.session.lock() {
                            session.load_from_buckets(&document.buckets);
                        }
                    }
                    Err(error) => {
                        self.storage_healthy = false;
                        crate::logging::info(format!("Failed to load stats: {error}"));
                    }
                },
                Err(error) => {
                    self.storage_healthy = false;
                    crate::logging::info(format!("Failed to parse stats: {error}"));
                }
            },
            Err(error) => {
                self.storage_healthy = false;
                crate::logging::info(format!("Failed to read stats: {error}"));
            }
        }
    }
}

fn process_name(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_migrates_old_stats_json_and_persists_the_new_schema() {
        let path = std::env::temp_dir().join(format!(
            "omnispot-stats-migration-{}.json",
            std::process::id()
        ));
        let old = serde_json::json!({
            "buckets": [{ "date": "0001-01-01", "total_switches": 0,
                "total_time_saved_ms": 0, "total_peeks": 0, "static_app_usage": {},
                "dynamic_app_usage": {}, "alt_tab_switches": 0,
                "alt_tab_keystrokes": 0, "fastest_switch": null }]
        });
        fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();

        let mut service = StatsService::new(&path);
        service.start(true);

        let migrated: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            migrated["schema_version"],
            super::super::migrations::CURRENT_VERSION
        );
        assert!(migrated["buckets"].as_array().unwrap().is_empty());
        assert!(
            service
                .snapshots()
                .iter()
                .all(|bucket| bucket.date != "0001-01-01")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn newer_stats_schema_is_left_untouched_and_collection_stays_disabled() {
        let path = std::env::temp_dir().join(format!(
            "omnispot-stats-future-schema-{}.json",
            std::process::id()
        ));
        let source = br#"{"schema_version":99,"buckets":[]}"#;
        fs::write(&path, source).unwrap();

        let mut service = StatsService::new(&path);
        service.start(true);

        assert!(!service.enabled);
        assert_eq!(fs::read(&path).unwrap(), source);
        let _ = fs::remove_file(path);
    }
}
