use super::*;
use crate::configuration::app_theme_setting::AppThemeSetting;
use crate::configuration::configuration::CycleMode;
use crate::configuration::packaged_app_path_sanitizer::PackagedAppInfo;
use crate::configuration::storage::SettingsDocument;
use crate::input::keyboard_input::LEFT_CTRL;

struct MockPackagedAppsService {
    package_dir: String,
    info: Option<PackagedAppInfo>,
}

impl IPackagedAppsService for MockPackagedAppsService {
    fn get_by_package_family_name(&self, _family_name: &str) -> Option<PackagedAppInfo> {
        None
    }

    fn get_installed_path_by_aumid(&self, _aumid: &str) -> Option<String> {
        None
    }

    fn get_by_installed_path(&self, process_path: &str) -> Option<PackagedAppInfo> {
        if process_path.starts_with(&self.package_dir) {
            self.info.clone()
        } else {
            None
        }
    }
}

fn seed_settings(apps: Vec<ApplicationConfigurationDocument>) -> ConfigurationDatabase {
    ConfigurationDatabase {
        settings: Some(SettingsDocument {
            id: 1,
            modifier: LEFT_CTRL,
            applications: apps,
            pulse_border_enabled: true,
            theme: AppThemeSetting::Dark,
            overlay_enabled: true,
            overlay_show_delay_ms: 1000,
            overlay_keep_open_while_modifier_held: true,
            peek_enabled: false,
            dynamic_mode_enabled: true,
            stats_enabled: false,
            spotlight_enabled: true,
            spotlight_modifier: crate::input::keyboard_input::LEFT_ALT,
            spotlight_custom_directories: None,
        }),
        migrations: Vec::new(),
    }
}

#[test]
fn version_is_one() {
    let service = MockPackagedAppsService {
        package_dir: String::new(),
        info: None,
    };
    let sut = FixPackagedAppsMigration::new(service);
    assert_eq!(sut.version(), 1);
}

#[test]
fn up_does_not_fail_when_database_is_empty() {
    let service = MockPackagedAppsService {
        package_dir: String::new(),
        info: None,
    };
    let sut = FixPackagedAppsMigration::new(service);
    let mut db = ConfigurationDatabase::default();
    assert!(sut.up(&mut db).is_ok());
}

#[test]
fn up_preserves_other_settings_after_migration() {
    let package_dir = r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_1.0_x64__8wekyb3d8bbwe";
    let exe_path = format!(r"{}\WindowsTerminal.exe", package_dir);

    let service = MockPackagedAppsService {
        package_dir: package_dir.to_string(),
        info: Some(PackagedAppInfo {
            aumid: "Microsoft.WindowsTerminal_8wekyb3d8bbwe!App".to_string(),
            icon_path: None,
        }),
    };
    let sut = FixPackagedAppsMigration::new(service);

    let mut db = seed_settings(vec![ApplicationConfigurationDocument {
        key: 44,
        process_path: exe_path,
        cycle_mode: CycleMode::NextApp,
        start_if_not_running: false,
        r#type: ApplicationType::Win32,
        aumid: None,
    }]);

    assert!(sut.up(&mut db).is_ok());

    let settings = db.settings.unwrap();
    assert_eq!(settings.applications.len(), 1);
    assert_eq!(settings.applications[0].r#type, ApplicationType::Packaged);
    assert_eq!(
        settings.applications[0].aumid.as_deref(),
        Some("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App")
    );
    assert!(settings.pulse_border_enabled);
    assert_eq!(settings.theme, AppThemeSetting::Dark);
}
