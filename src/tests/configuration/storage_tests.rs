use crate::configuration::app_theme_setting::AppThemeSetting;
use crate::configuration::application_type::ApplicationType;
use crate::configuration::configuration::{ApplicationConfiguration, Configuration, CycleMode};
use crate::configuration::storage::{ApplicationConfigurationDocument, SettingsDocument};
use crate::input::keyboard_input::{LEFT_ALT, LEFT_SHIFT, RIGHT_CTRL};

const KEY_W: u32 = 66;
const KEY_T: u32 = 63;
const KEY_N: u32 = 57;
const KEY_B: u32 = 45;
const KEY_E: u32 = 48;

#[test]
fn application_configuration_document_to_application_configuration_maps_all_fields() {
    let doc = ApplicationConfigurationDocument {
        key: KEY_W,
        process_path: r"C:\tools\app.exe".to_string(),
        cycle_mode: CycleMode::NextWindow,
        start_if_not_running: true,
        r#type: ApplicationType::Win32,
        aumid: None,
    };

    let result = doc.to_application_configuration();
    assert_eq!(
        result,
        ApplicationConfiguration {
            key: KEY_W,
            process_path: r"C:\tools\app.exe".to_string(),
            cycle_mode: CycleMode::NextWindow,
            start_if_not_running: true,
            application_type: ApplicationType::Win32,
            aumid: None,
        }
    );
}

#[test]
fn application_configuration_document_to_application_configuration_maps_packaged_fields() {
    let doc = ApplicationConfigurationDocument {
        key: KEY_T,
        process_path: "WindowsTerminal.exe".to_string(),
        cycle_mode: CycleMode::Hide,
        start_if_not_running: true,
        r#type: ApplicationType::Packaged,
        aumid: Some("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App".to_string()),
    };

    let result = doc.to_application_configuration();
    assert_eq!(
        result,
        ApplicationConfiguration {
            key: KEY_T,
            process_path: "WindowsTerminal.exe".to_string(),
            cycle_mode: CycleMode::Hide,
            start_if_not_running: true,
            application_type: ApplicationType::Packaged,
            aumid: Some("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App".to_string()),
        }
    );
}

#[test]
fn settings_document_to_configuration_maps_all_fields() {
    let doc = SettingsDocument {
        id: 1,
        modifier: LEFT_ALT,
        applications: vec![ApplicationConfigurationDocument {
            key: KEY_N,
            process_path: r"C:\Windows\notepad.exe".to_string(),
            cycle_mode: CycleMode::NextApp,
            start_if_not_running: false,
            r#type: ApplicationType::Win32,
            aumid: None,
        }],
        pulse_border_enabled: false,
        theme: AppThemeSetting::Dark,
        overlay_enabled: true,
        overlay_show_delay_ms: 500,
        overlay_keep_open_while_modifier_held: true,
        peek_enabled: false,
        dynamic_mode_enabled: true,
        stats_enabled: true,
        spotlight_enabled: true,
        spotlight_modifier: LEFT_ALT,
        spotlight_custom_directories: None,
        hide_windows_taskbar: true,
        taskbar_modifier: crate::input::keyboard_input::LEFT_ALT,
    };

    let result = doc.to_configuration();
    assert_eq!(result.modifier, LEFT_ALT);
    assert_eq!(result.applications.len(), 1);
    assert_eq!(result.theme, AppThemeSetting::Dark);
    assert!(result.overlay_enabled);
    assert_eq!(result.overlay_show_delay_ms, 500);
    assert!(result.hide_windows_taskbar);
}

#[test]
fn settings_document_from_configuration_maps_all_fields() {
    let config = Configuration {
        modifier: RIGHT_CTRL,
        applications: vec![ApplicationConfiguration {
            key: KEY_B,
            process_path: r"C:\apps\browser.exe".to_string(),
            cycle_mode: CycleMode::Hide,
            start_if_not_running: true,
            application_type: ApplicationType::Win32,
            aumid: None,
        }],
        pulse_border_enabled: true,
        theme: AppThemeSetting::Light,
        overlay_enabled: true,
        overlay_show_delay_ms: 500,
        overlay_keep_open_while_modifier_held: true,
        peek_enabled: false,
        dynamic_mode_enabled: true,
        stats_enabled: true,
        spotlight_enabled: true,
        spotlight_modifier: LEFT_ALT,
        spotlight_custom_directories: None,
        hide_windows_taskbar: true,
        taskbar_modifier: crate::input::keyboard_input::LEFT_ALT,
    };

    let result = SettingsDocument::from_configuration(&config, 42);
    assert_eq!(result.id, 42);
    assert_eq!(result.modifier, RIGHT_CTRL);
    assert_eq!(result.applications.len(), 1);
    assert_eq!(result.applications[0].key, KEY_B);
    assert_eq!(result.theme, AppThemeSetting::Light);
    assert!(result.hide_windows_taskbar);
}

#[test]
fn settings_document_round_trip_preserves_all_data() {
    let original = Configuration {
        modifier: LEFT_SHIFT,
        applications: vec![
            ApplicationConfiguration {
                key: KEY_E,
                process_path: r"C:\apps\editor.exe".to_string(),
                cycle_mode: CycleMode::NextWindow,
                start_if_not_running: false,
                application_type: ApplicationType::Win32,
                aumid: None,
            },
            ApplicationConfiguration {
                key: KEY_T,
                process_path: r"C:\apps\terminal.exe".to_string(),
                cycle_mode: CycleMode::NextApp,
                start_if_not_running: true,
                application_type: ApplicationType::Win32,
                aumid: None,
            },
        ],
        pulse_border_enabled: true,
        theme: AppThemeSetting::System,
        overlay_enabled: true,
        overlay_show_delay_ms: 500,
        overlay_keep_open_while_modifier_held: true,
        peek_enabled: false,
        dynamic_mode_enabled: true,
        stats_enabled: true,
        spotlight_enabled: true,
        spotlight_modifier: LEFT_ALT,
        spotlight_custom_directories: None,
        hide_windows_taskbar: true,
        taskbar_modifier: crate::input::keyboard_input::LEFT_ALT,
    };

    let round_tripped = SettingsDocument::from_configuration(&original, 1).to_configuration();
    assert_eq!(round_tripped, original);
}
