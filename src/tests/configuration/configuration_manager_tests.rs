use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::configuration::app_theme_setting::AppThemeSetting;
use crate::configuration::configuration::{ApplicationConfiguration, Configuration, CycleMode};
use crate::configuration::configuration_manager::ConfigurationManager;
use crate::configuration::configuration_service::ConfigurationService;
use crate::configuration::migrations::i_migration::ConfigurationDatabase;
use crate::configuration::packaged_app_path_sanitizer::{IPackagedAppsService, PackagedAppInfo};
use crate::input::keyboard_input::{LEFT_ALT, RIGHT_CTRL};

struct DummyPackagedApps;
impl IPackagedAppsService for DummyPackagedApps {
    fn get_by_package_family_name(&self, _: &str) -> Option<PackagedAppInfo> { None }
    fn get_installed_path_by_aumid(&self, _: &str) -> Option<String> { None }
    fn get_by_installed_path(&self, _: &str) -> Option<PackagedAppInfo> { None }
}

fn make_test_config() -> Configuration {
    Configuration {
        modifier: RIGHT_CTRL,
        applications: vec![],
        pulse_border_enabled: true,
        theme: AppThemeSetting::System,
        overlay_enabled: false,
        overlay_show_delay_ms: 1000,
        overlay_keep_open_while_modifier_held: true,
        peek_enabled: false,
        dynamic_mode_enabled: true,
        stats_enabled: false,
        spotlight_enabled: true,
        spotlight_modifier: LEFT_ALT,
        spotlight_custom_directories: None,
        hide_windows_taskbar: false,
        taskbar_modifier: crate::input::keyboard_input::LEFT_ALT,
    }
}

#[test]
fn get_configuration_runs_migrations_and_loads_config() {
    let db = Arc::new(Mutex::new(ConfigurationDatabase::default()));
    let config_svc = Arc::new(ConfigurationService::new(db.clone()));
    let pkg_apps = Arc::new(DummyPackagedApps);

    let sut = ConfigurationManager::new(config_svc, pkg_apps, db.clone());

    let config = sut.get_configuration();
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.modifier, RIGHT_CTRL);

    // Migrations should have been executed
    let db_guard = db.lock().unwrap();
    assert!(!db_guard.migrations.is_empty());
}

#[test]
fn update_configuration_persists_and_notifies_listener() {
    let db = Arc::new(Mutex::new(ConfigurationDatabase::default()));
    let config_svc = Arc::new(ConfigurationService::new(db.clone()));
    let pkg_apps = Arc::new(DummyPackagedApps);

    let sut = ConfigurationManager::new(config_svc, pkg_apps, db.clone());

    let notification_count = Arc::new(AtomicI32::new(0));
    let counter_clone = notification_count.clone();

    sut.on_configuration_changed(move |_cfg| {
        counter_clone.fetch_add(1, Ordering::SeqCst);
    });

    let mut new_config = make_test_config();
    new_config.applications.push(ApplicationConfiguration::new(
        44,
        "C:\\notepad.exe".to_string(),
        CycleMode::NextApp,
        false,
    ));

    let updated = sut.update_configuration(new_config.clone());
    assert!(updated);
    assert_eq!(notification_count.load(Ordering::SeqCst), 1);

    let retrieved = sut.get_configuration().unwrap();
    assert_eq!(retrieved.applications.len(), 1);
    assert_eq!(retrieved.applications[0].process_path, "C:\\notepad.exe");
}
