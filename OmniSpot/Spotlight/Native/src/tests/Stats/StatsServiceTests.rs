use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;

use crate::configuration::app_theme_setting::AppThemeSetting;
use crate::configuration::configuration::Configuration;
use crate::stats::app_registry_cache::{
    AppRegistryCache, IPackagedAppsProvider, IProcessInspectorProvider,
};
use crate::stats::session_stats::SessionStats;
use crate::stats::stats_event::StatsEvent;
use crate::stats::stats_service::StatsService;
use crate::stats::storage::{DailyBucketDocument, StatsDatabase};
use crate::window_discovery::application_window::ApplicationWindow;
use crate::window_discovery::i_window_enumerator::IWindowEnumerator;
use crate::window_discovery::window_enumerator::FocusedAppWindows;
use crate::configuration::configuration::ApplicationConfiguration;

struct DummyWindowEnumerator;
impl IWindowEnumerator for DummyWindowEnumerator {
    fn get_windows(&self) -> Vec<ApplicationWindow> { vec![] }
    fn get_cached_windows(&mut self) -> Vec<ApplicationWindow> { vec![] }
    fn get_current_window(&self) -> Option<ApplicationWindow> { None }
    fn get_focused_app_windows(&self, _: &[ApplicationWindow], _: &[ApplicationConfiguration]) -> FocusedAppWindows {
        FocusedAppWindows::default()
    }
    fn get_all_windows(&self) -> Vec<ApplicationWindow> { vec![] }
}

struct DummyPackagedApps;
impl IPackagedAppsProvider for DummyPackagedApps {
    fn get_installed_packages(&self) -> Vec<String> { vec![] }
    fn get_display_name(&self, _: &str) -> Option<String> { None }
}

struct DummyProcessInspector;
impl IProcessInspectorProvider for DummyProcessInspector {
    fn get_process_display_name(&self, path: &str) -> String { path.to_string() }
}

fn create_sut(db: Arc<Mutex<StatsDatabase>>, db_exists: bool) -> (StatsService, Arc<SessionStats>) {
    let session_stats = Arc::new(SessionStats::new());
    let cache = Arc::new(AppRegistryCache::new(
        db.clone(),
        Arc::new(DummyWindowEnumerator),
        Arc::new(DummyPackagedApps),
        Arc::new(DummyProcessInspector),
    ));

    let service = StatsService::new(session_stats.clone(), cache, db, db_exists);
    (service, session_stats)
}

fn make_config(stats_enabled: bool) -> Configuration {
    Configuration {
        modifier: 118,
        applications: vec![],
        pulse_border_enabled: false,
        theme: AppThemeSetting::System,
        overlay_enabled: false,
        overlay_show_delay_ms: 0,
        overlay_keep_open_while_modifier_held: false,
        peek_enabled: false,
        dynamic_mode_enabled: false,
        stats_enabled,
        spotlight_enabled: false,
        spotlight_modifier: 0,
        spotlight_custom_directories: None,
    }
}

#[test]
fn start_disabled_does_not_start_consumer() {
    let db = Arc::new(Mutex::new(StatsDatabase::default()));
    let (sut, _) = create_sut(db, true);

    sut.start(false);
    assert!(!sut.is_running());
}

#[test]
fn start_enabled_starts_consumer_and_loads_todays_bucket() {
    let mut initial_db = StatsDatabase::default();
    let today = crate::stats::session_stats::system_today();
    let today_bucket = DailyBucketDocument {
        date: today.clone(),
        total_switches: 42,
        ..Default::default()
    };
    initial_db.daily_buckets.insert(today, serde_json::to_value(&today_bucket).unwrap());

    let db = Arc::new(Mutex::new(initial_db));
    let (sut, session_stats) = create_sut(db, true);

    sut.start(true);
    assert!(sut.is_running());

    let snapshots = session_stats.get_snapshots();
    assert_eq!(snapshots[0].total_switches, 42);

    sut.stop_consumer("test end");
    assert!(!sut.is_running());
}

#[test]
fn update_configuration_toggles_running_state() {
    let db = Arc::new(Mutex::new(StatsDatabase::default()));
    let (sut, _) = create_sut(db, true);

    sut.start(false);
    assert!(!sut.is_running());

    sut.update_configuration(&make_config(true));
    assert!(sut.is_running());

    sut.update_configuration(&make_config(false));
    assert!(!sut.is_running());
}

#[test]
fn enqueue_and_flush_persists_to_database() {
    let db = Arc::new(Mutex::new(StatsDatabase::default()));
    let (sut, _) = create_sut(db.clone(), true);

    sut.start(true);

    sut.enqueue(StatsEvent::AltTabEvent(3));

    // Give consumer thread a moment to process
    thread::sleep(Duration::from_millis(50));

    sut.flush("test flush");

    {
        let db_guard = db.lock().unwrap();
        let today = crate::stats::session_stats::system_today();
        assert!(db_guard.daily_buckets.contains_key(&today));
    }

    sut.stop_consumer("done");
}
