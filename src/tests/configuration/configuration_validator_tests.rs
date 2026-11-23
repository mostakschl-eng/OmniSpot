use super::*;
use crate::configuration::app_theme_setting::AppThemeSetting;
use crate::configuration::configuration::CycleMode;
use crate::input::keyboard_input::{D0, D1, NONE, RETURN, SPACE};

const F1: u32 = 90;

fn make_config(modifier: u32, apps: Vec<ApplicationConfiguration>) -> Configuration {
    Configuration {
        modifier,
        applications: apps,
        pulse_border_enabled: true,
        theme: AppThemeSetting::System,
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

fn make_app(key: u32, process_path: &str) -> ApplicationConfiguration {
    ApplicationConfiguration::new(key, process_path.to_string(), CycleMode::NextApp, false)
}

fn make_packaged_app(key: u32, exe_filename: &str, aumid: Option<&str>) -> ApplicationConfiguration {
    ApplicationConfiguration {
        key,
        process_path: exe_filename.to_string(),
        cycle_mode: CycleMode::NextApp,
        start_if_not_running: false,
        application_type: ApplicationType::Packaged,
        aumid: aumid.map(|s| s.to_string()),
    }
}

#[test]
fn validate_and_log_returns_success_when_no_applications_configured() {
    let result = ConfigurationValidator::validate(&make_config(LEFT_CTRL, vec![]));
    assert_eq!(result.status, ValidationResultStatus::Success);
}

#[test]
fn validate_and_log_returns_success_for_each_valid_modifier() {
    for &modifier in &VALID_MODIFIERS {
        let result = ConfigurationValidator::validate(&make_config(modifier, vec![]));
        assert_eq!(result.status, ValidationResultStatus::Success);
    }
}

#[test]
fn validate_and_log_returns_error_when_modifier_is_not_in_allowed_set() {
    let invalid_modifiers = [A, B_KEY, NONE, F1, D0, RETURN];
    for modifier in invalid_modifiers {
        let result = ConfigurationValidator::validate(&make_config(modifier, vec![]));
        assert_eq!(result.status, ValidationResultStatus::Error);
        assert!(result.message.unwrap().contains("Invalid modifier key"));
    }
}

const B_KEY: u32 = 45;

#[test]
fn validate_and_log_returns_error_when_process_path_is_empty() {
    for process_path in ["", "   "] {
        let result = ConfigurationValidator::validate(&make_config(
            LEFT_CTRL,
            vec![make_app(A, process_path)],
        ));
        assert_eq!(result.status, ValidationResultStatus::Error);
        assert!(result.message.unwrap().contains("process path missing or invalid"));
    }
}

#[test]
fn validate_and_log_returns_error_when_process_file_does_not_exist() {
    let result = ConfigurationValidator::validate(&make_config(
        LEFT_CTRL,
        vec![make_app(A, r"C:\nonexistent\fake_app_12345.exe")],
    ));
    assert_eq!(result.status, ValidationResultStatus::Error);
    assert!(result.message.unwrap().contains("process path missing or invalid"));
}

#[test]
fn validate_and_log_returns_error_when_application_key_is_not_a_letter() {
    let temp_file = std::env::temp_dir().join("omnispot_test_temp_app.exe");
    std::fs::write(&temp_file, b"test").unwrap();

    let invalid_keys = [NONE, D1, F1, RETURN, SPACE];
    for key in invalid_keys {
        let result = ConfigurationValidator::validate(&make_config(
            LEFT_CTRL,
            vec![make_app(key, temp_file.to_str().unwrap())],
        ));
        assert_eq!(result.status, ValidationResultStatus::Error);
        assert!(result.message.unwrap().contains("single letter"));
    }

    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn validate_and_log_returns_success_when_application_key_is_a_letter() {
    let temp_file = std::env::temp_dir().join("omnispot_test_temp_app_letter.exe");
    std::fs::write(&temp_file, b"test").unwrap();

    let letter_keys = [A, 56, Z]; // A, M, Z
    for key in letter_keys {
        let result = ConfigurationValidator::validate(&make_config(
            LEFT_CTRL,
            vec![make_app(key, temp_file.to_str().unwrap())],
        ));
        assert_eq!(result.status, ValidationResultStatus::Success);
    }

    let _ = std::fs::remove_file(temp_file);
}

#[test]
fn validate_and_log_returns_success_for_packaged_app_with_valid_aumid() {
    let app = make_packaged_app(A, "WindowsTerminal.exe", Some("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"));
    let result = ConfigurationValidator::validate(&make_config(LEFT_CTRL, vec![app]));
    assert_eq!(result.status, ValidationResultStatus::Success);
}

#[test]
fn validate_and_log_returns_error_for_packaged_app_when_aumid_is_missing_or_whitespace() {
    let cases = [None, Some(""), Some("   ")];
    for aumid in cases {
        let app = make_packaged_app(A, "WindowsTerminal.exe", aumid);
        let result = ConfigurationValidator::validate(&make_config(LEFT_CTRL, vec![app]));
        assert_eq!(result.status, ValidationResultStatus::Error);
        assert!(result.message.unwrap().contains("AUMID"));
    }
}

#[test]
fn validate_and_log_returns_error_when_spotlight_modifier_is_invalid() {
    let mut config = make_config(LEFT_CTRL, vec![]);
    config.spotlight_modifier = A;
    let result = ConfigurationValidator::validate(&config);
    assert_eq!(result.status, ValidationResultStatus::Error);
    assert!(result.message.unwrap().contains("Invalid spotlight modifier key"));
}
