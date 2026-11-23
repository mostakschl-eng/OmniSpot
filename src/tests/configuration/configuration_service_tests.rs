use std::sync::{Arc, Mutex};
use crate::configuration::app_theme_setting::AppThemeSetting;
use crate::configuration::configuration::{ApplicationConfiguration, Configuration, CycleMode};
use crate::configuration::configuration_service::ConfigurationService;
use crate::configuration::migrations::i_migration::ConfigurationDatabase;
use crate::input::keyboard_input::{LEFT_ALT, RIGHT_ALT, RIGHT_CTRL};

fn make_config(
    modifier: u32,
    applications: Vec<ApplicationConfiguration>,
    theme: AppThemeSetting,
) -> Configuration {
    Configuration {
        modifier,
        applications,
        pulse_border_enabled: true,
        theme,
        overlay_enabled: true,
        overlay_show_delay_ms: 1000,
        overlay_keep_open_while_modifier_held: true,
        peek_enabled: false,
        dynamic_mode_enabled: false,
        stats_enabled: true,
        spotlight_enabled: true,
        spotlight_modifier: LEFT_ALT,
        spotlight_custom_directories: None,
        hide_windows_taskbar: false,
        taskbar_modifier: crate::input::keyboard_input::LEFT_ALT,
    }
}

#[test]
fn read_configuration_returns_defaults_when_no_database_record_exists() {
    let db = Arc::new(Mutex::new(ConfigurationDatabase::default()));
    let sut = ConfigurationService::new(db);

    let result = sut.read_configuration();

    assert_eq!(result.modifier, RIGHT_CTRL);
    assert!(result.applications.is_empty());
    assert!(result.pulse_border_enabled);
    assert_eq!(result.theme, AppThemeSetting::System);
    assert!(!result.overlay_enabled);
    assert_eq!(result.overlay_show_delay_ms, 1000);
    assert!(result.overlay_keep_open_while_modifier_held);
    assert!(!result.peek_enabled);
    assert!(result.dynamic_mode_enabled);
    assert!(!result.stats_enabled);
    assert!(result.spotlight_enabled);
    assert_eq!(result.spotlight_modifier, LEFT_ALT);
}

#[test]
fn read_configuration_returns_stored_configuration_after_write() {
    let db = Arc::new(Mutex::new(ConfigurationDatabase::default()));
    let sut = ConfigurationService::new(db);

    let app = ApplicationConfiguration::new(
        57, // Key.N
        "C:\\Windows\\notepad.exe".to_string(),
        CycleMode::Hide,
        false,
    );
    let config = make_config(RIGHT_CTRL, vec![app], AppThemeSetting::System);

    sut.write_configuration(&config);
    let result = sut.read_configuration();

    assert_eq!(result, config);
}

#[test]
fn write_configuration_overwrites_when_called_twice() {
    let db = Arc::new(Mutex::new(ConfigurationDatabase::default()));
    let sut = ConfigurationService::new(db);

    let first = make_config(RIGHT_CTRL, vec![], AppThemeSetting::Light);
    let second = make_config(RIGHT_ALT, vec![], AppThemeSetting::Dark);

    sut.write_configuration(&first);
    sut.write_configuration(&second);
    let result = sut.read_configuration();

    assert_eq!(result, second);
}

#[test]
fn read_configuration_preserves_multiple_applications() {
    let db = Arc::new(Mutex::new(ConfigurationDatabase::default()));
    let sut = ConfigurationService::new(db);

    let apps = vec![
        ApplicationConfiguration::new(44, "C:\\apps\\a.exe".to_string(), CycleMode::NextApp, false),
        ApplicationConfiguration::new(45, "C:\\apps\\b.exe".to_string(), CycleMode::Hide, true),
        ApplicationConfiguration::new(46, "C:\\apps\\c.exe".to_string(), CycleMode::NextWindow, false),
    ];
    let config = make_config(RIGHT_CTRL, apps, AppThemeSetting::System);

    sut.write_configuration(&config);
    let result = sut.read_configuration();

    assert_eq!(result.applications.len(), 3);
    assert_eq!(result.applications[0].process_path, "C:\\apps\\a.exe");
    assert_eq!(result.applications[1].process_path, "C:\\apps\\b.exe");
    assert_eq!(result.applications[2].process_path, "C:\\apps\\c.exe");
}
