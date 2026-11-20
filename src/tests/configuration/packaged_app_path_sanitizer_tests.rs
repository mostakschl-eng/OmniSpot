use super::*;
use crate::configuration::app_theme_setting::AppThemeSetting;
use crate::configuration::configuration::CycleMode;
use crate::input::keyboard_input::{A, LEFT_ALT, RIGHT_CTRL};

const C_KEY: u32 = 46; // C key in WPF
const CLAUDE_PUBLISHER_ID: &str = "pzs8sxrjxfjjc";
const CLAUDE_FAMILY_NAME: &str = "Claude_pzs8sxrjxfjjc";
const CLAUDE_AUMID: &str = "Claude_pzs8sxrjxfjjc!Claude";
const STALE_CLAUDE_PACKAGE: &str = r"C:\Program Files\WindowsApps\Claude_0.0.1.0_x64__pzs8sxrjxfjjc";
const NEW_CLAUDE_PACKAGE: &str = r"C:\Program Files\WindowsApps\Claude_0.0.2.0_x64__pzs8sxrjxfjjc";

struct FakePackagedAppsService {
    installed_path: Option<String>,
    app_info: Option<PackagedAppInfo>,
}

impl FakePackagedAppsService {
    fn new() -> Self {
        Self {
            installed_path: Some(NEW_CLAUDE_PACKAGE.to_string()),
            app_info: Some(PackagedAppInfo {
                aumid: CLAUDE_AUMID.to_string(),
                icon_path: None,
            }),
        }
    }
}

impl IPackagedAppsService for FakePackagedAppsService {
    fn get_by_package_family_name(&self, family_name: &str) -> Option<PackagedAppInfo> {
        if family_name.eq_ignore_ascii_case(CLAUDE_FAMILY_NAME) {
            self.app_info.clone()
        } else {
            None
        }
    }

    fn get_installed_path_by_aumid(&self, aumid: &str) -> Option<String> {
        if aumid.eq_ignore_ascii_case(CLAUDE_AUMID) {
            self.installed_path.clone()
        } else {
            None
        }
    }
}

fn config_with(apps: Vec<ApplicationConfiguration>) -> Configuration {
    Configuration {
        modifier: RIGHT_CTRL,
        applications: apps,
        pulse_border_enabled: false,
        theme: AppThemeSetting::System,
        overlay_enabled: false,
        overlay_show_delay_ms: 0,
        overlay_keep_open_while_modifier_held: false,
        peek_enabled: false,
        dynamic_mode_enabled: false,
        stats_enabled: false,
        spotlight_enabled: true,
        spotlight_modifier: LEFT_ALT,
        spotlight_custom_directories: None,
    }
}

fn packaged_app(process_path: &str, aumid: &str) -> ApplicationConfiguration {
    ApplicationConfiguration {
        key: C_KEY,
        process_path: process_path.to_string(),
        cycle_mode: CycleMode::NextApp,
        start_if_not_running: false,
        application_type: ApplicationType::Packaged,
        aumid: Some(aumid.to_string()),
    }
}

fn win32_app(process_path: &str) -> ApplicationConfiguration {
    ApplicationConfiguration::new(
        C_KEY,
        process_path.to_string(),
        CycleMode::NextApp,
        false,
    )
}

#[test]
fn sanitize_keeps_package_subdirectory_when_executable_is_not_in_package_root() {
    let service = FakePackagedAppsService::new();
    let sut = PackagedAppPathSanitizer::new(service);

    let stale_path = format!(r"{}\app\Claude.exe", STALE_CLAUDE_PACKAGE);
    let expected_path = format!(r"{}\app\Claude.exe", NEW_CLAUDE_PACKAGE);

    let config = config_with(vec![packaged_app(&stale_path, CLAUDE_AUMID)]);
    let result = sut.sanitize(&config);

    assert!(result.is_some());
    let sanitized = result.unwrap();
    assert_eq!(sanitized.applications[0].process_path, expected_path);
}

#[test]
fn sanitize_updates_path_when_executable_is_in_package_root() {
    let service = FakePackagedAppsService::new();
    let sut = PackagedAppPathSanitizer::new(service);

    let stale_path = format!(r"{}\Claude.exe", STALE_CLAUDE_PACKAGE);
    let expected_path = format!(r"{}\Claude.exe", NEW_CLAUDE_PACKAGE);

    let config = config_with(vec![packaged_app(&stale_path, CLAUDE_AUMID)]);
    let result = sut.sanitize(&config);

    assert!(result.is_some());
    let sanitized = result.unwrap();
    assert_eq!(sanitized.applications[0].process_path, expected_path);
}

#[test]
fn sanitize_falls_back_to_executable_name_when_stale_path_is_under_different_root() {
    let service = FakePackagedAppsService::new();
    let sut = PackagedAppPathSanitizer::new(service);

    let stale_path = r"D:\WindowsApps\Claude_0.0.1.0_x64__pzs8sxrjxfjjc\app\Claude.exe";
    let expected_path = format!(r"{}\Claude.exe", NEW_CLAUDE_PACKAGE);

    let config = config_with(vec![packaged_app(stale_path, CLAUDE_AUMID)]);
    let result = sut.sanitize(&config);

    assert!(result.is_some());
    let sanitized = result.unwrap();
    assert_eq!(sanitized.applications[0].process_path, expected_path);
}

#[test]
fn sanitize_returns_none_when_package_cannot_be_resolved() {
    let mut service = FakePackagedAppsService::new();
    service.installed_path = None;
    let sut = PackagedAppPathSanitizer::new(service);

    let stale_path = format!(r"{}\app\Claude.exe", STALE_CLAUDE_PACKAGE);
    let config = config_with(vec![packaged_app(&stale_path, CLAUDE_AUMID)]);
    let result = sut.sanitize(&config);

    assert!(result.is_none());
}

#[test]
fn sanitize_returns_none_when_no_packaged_apps() {
    let service = FakePackagedAppsService::new();
    let sut = PackagedAppPathSanitizer::new(service);

    let config = config_with(vec![ApplicationConfiguration::new(
        A,
        r"C:\missing\notepad.exe".to_string(),
        CycleMode::NextApp,
        false,
    )]);
    let result = sut.sanitize(&config);

    assert!(result.is_none());
}

#[test]
fn sanitize_converts_to_packaged_and_fixes_path_when_win32_app_has_stale_package_path() {
    let service = FakePackagedAppsService::new();
    let sut = PackagedAppPathSanitizer::new(service);

    let stale_path = format!(r"{}\app\claude.exe", STALE_CLAUDE_PACKAGE);
    let expected_path = format!(r"{}\app\claude.exe", NEW_CLAUDE_PACKAGE);

    let config = config_with(vec![win32_app(&stale_path)]);
    let result = sut.sanitize(&config);

    assert!(result.is_some());
    let app = &result.unwrap().applications[0];
    assert_eq!(app.application_type, ApplicationType::Packaged);
    assert_eq!(app.aumid.as_deref(), Some(CLAUDE_AUMID));
    assert_eq!(app.process_path, expected_path);
}

#[test]
fn sanitize_does_not_look_up_package_when_win32_app_is_not_in_package_dir() {
    let service = FakePackagedAppsService::new();
    let sut = PackagedAppPathSanitizer::new(service);

    let non_package_paths = [
        r"C:\Program Files\Notepad++\notepad++.exe",
        r"C:\Program Files\WindowsApps\Claude.exe",
        r"C:\Tools\my_tool_v1_x64_build\tool.exe",
    ];

    for path in non_package_paths {
        let config = config_with(vec![win32_app(path)]);
        let result = sut.sanitize(&config);
        assert!(result.is_none());
    }
}
