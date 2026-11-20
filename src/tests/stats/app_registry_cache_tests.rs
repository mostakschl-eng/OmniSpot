use std::sync::{Arc, Mutex};
use crate::configuration::app_theme_setting::AppThemeSetting;
use crate::configuration::configuration::{ApplicationConfiguration, Configuration, CycleMode};
use crate::stats::app_registry_cache::{
    AppRegistryCache, IPackagedAppsProvider, IProcessInspectorProvider,
};
use crate::stats::storage::StatsDatabase;
use crate::window_discovery::application_window::ApplicationWindow;
use crate::window_discovery::i_window_enumerator::IWindowEnumerator;
use crate::window_discovery::window_enumerator::FocusedAppWindows;

struct MockWindowEnumerator {
    windows: Vec<ApplicationWindow>,
}

impl IWindowEnumerator for MockWindowEnumerator {
    fn get_windows(&self) -> Vec<ApplicationWindow> {
        self.windows.clone()
    }
    fn get_cached_windows(&mut self) -> Vec<ApplicationWindow> {
        self.windows.clone()
    }
    fn get_current_window(&self) -> Option<ApplicationWindow> {
        None
    }
    fn get_focused_app_windows(
        &self,
        _all: &[ApplicationWindow],
        _applications: &[ApplicationConfiguration],
    ) -> FocusedAppWindows {
        FocusedAppWindows::default()
    }
    fn get_all_windows(&self) -> Vec<ApplicationWindow> {
        self.windows.clone()
    }
}

struct MockPackagedAppsProvider {
    installed_packages: Vec<String>,
}

impl IPackagedAppsProvider for MockPackagedAppsProvider {
    fn get_installed_packages(&self) -> Vec<String> {
        self.installed_packages.clone()
    }
    fn get_display_name(&self, _path: &str) -> Option<String> {
        None
    }
}

struct MockProcessInspector {
    display_names: Mutex<std::collections::HashMap<String, String>>,
    call_count: Mutex<usize>,
}

impl IProcessInspectorProvider for MockProcessInspector {
    fn get_process_display_name(&self, path: &str) -> String {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        let map = self.display_names.lock().unwrap();
        map.get(path).cloned().unwrap_or_else(|| "DefaultApp".to_string())
    }
}

fn empty_config() -> Configuration {
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
        stats_enabled: false,
        spotlight_enabled: false,
        spotlight_modifier: 0,
        spotlight_custom_directories: None,
    }
}

fn make_window(process_image_path: &str) -> ApplicationWindow {
    ApplicationWindow {
        process_image_path: process_image_path.to_string(),
        ..Default::default()
    }
}

#[test]
fn get_display_name_returns_file_name_without_extension_when_not_in_cache() {
    let db = Arc::new(Mutex::new(StatsDatabase::default()));
    let win_enum = Arc::new(MockWindowEnumerator { windows: vec![] });
    let pkg_apps = Arc::new(MockPackagedAppsProvider { installed_packages: vec![] });
    let inspector = Arc::new(MockProcessInspector {
        display_names: Mutex::new(std::collections::HashMap::new()),
        call_count: Mutex::new(0),
    });

    let sut = AppRegistryCache::new(db, win_enum, pkg_apps, inspector);
    let result = sut.get_display_name("notepad.exe", None);
    assert_eq!(result, "notepad");
}

#[test]
fn get_display_name_is_case_insensitive_for_process_name() {
    let db = Arc::new(Mutex::new(StatsDatabase::default()));
    db.lock()
        .unwrap()
        .app_registry
        .insert("notepad.exe".to_string(), "Notepad".to_string());

    let win_enum = Arc::new(MockWindowEnumerator { windows: vec![] });
    let pkg_apps = Arc::new(MockPackagedAppsProvider { installed_packages: vec![] });
    let inspector = Arc::new(MockProcessInspector {
        display_names: Mutex::new(std::collections::HashMap::new()),
        call_count: Mutex::new(0),
    });

    let sut = AppRegistryCache::new(db, win_enum, pkg_apps, inspector);
    sut.prepopulate(&empty_config());

    let result = sut.get_display_name("NOTEPAD.EXE", None);
    assert_eq!(result, "Notepad");
}

#[test]
fn try_add_returns_false_when_process_already_in_cache() {
    let db = Arc::new(Mutex::new(StatsDatabase::default()));
    db.lock()
        .unwrap()
        .app_registry
        .insert("notepad.exe".to_string(), "Notepad".to_string());

    let win_enum = Arc::new(MockWindowEnumerator { windows: vec![] });
    let pkg_apps = Arc::new(MockPackagedAppsProvider { installed_packages: vec![] });
    let inspector = Arc::new(MockProcessInspector {
        display_names: Mutex::new(std::collections::HashMap::new()),
        call_count: Mutex::new(0),
    });

    let sut = AppRegistryCache::new(db, win_enum, pkg_apps, inspector);
    sut.prepopulate(&empty_config());

    let added = sut.try_add("notepad.exe", "C:\\notepad.exe");
    assert!(!added);
}

#[test]
fn try_add_persists_entry_to_db_on_cache_miss() {
    let db = Arc::new(Mutex::new(StatsDatabase::default()));
    let win_enum = Arc::new(MockWindowEnumerator { windows: vec![] });
    let pkg_apps = Arc::new(MockPackagedAppsProvider { installed_packages: vec![] });
    let mut names = std::collections::HashMap::new();
    names.insert("C:\\notepad.exe".to_string(), "Notepad".to_string());
    let inspector = Arc::new(MockProcessInspector {
        display_names: Mutex::new(names),
        call_count: Mutex::new(0),
    });

    let sut = AppRegistryCache::new(db.clone(), win_enum, pkg_apps, inspector);
    let added = sut.try_add("notepad.exe", "C:\\notepad.exe");
    assert!(added);

    let db_guard = db.lock().unwrap();
    assert_eq!(db_guard.app_registry.get("notepad.exe"), Some(&"Notepad".to_string()));
}

#[test]
fn prepopulate_seeds_cache_from_running_windows() {
    let db = Arc::new(Mutex::new(StatsDatabase::default()));
    let win_enum = Arc::new(MockWindowEnumerator {
        windows: vec![make_window("C:\\Windows\\notepad.exe")],
    });
    let pkg_apps = Arc::new(MockPackagedAppsProvider { installed_packages: vec![] });
    let mut names = std::collections::HashMap::new();
    names.insert("C:\\Windows\\notepad.exe".to_string(), "Notepad".to_string());
    let inspector = Arc::new(MockProcessInspector {
        display_names: Mutex::new(names),
        call_count: Mutex::new(0),
    });

    let sut = AppRegistryCache::new(db, win_enum, pkg_apps, inspector);
    sut.prepopulate(&empty_config());

    assert_eq!(sut.get_display_name("notepad.exe", None), "Notepad");
}

#[test]
fn prepopulate_seeds_cache_from_configured_apps() {
    let db = Arc::new(Mutex::new(StatsDatabase::default()));
    let win_enum = Arc::new(MockWindowEnumerator { windows: vec![] });
    let pkg_apps = Arc::new(MockPackagedAppsProvider { installed_packages: vec![] });
    let mut names = std::collections::HashMap::new();
    names.insert("C:\\apps\\code.exe".to_string(), "Visual Studio Code".to_string());
    let inspector = Arc::new(MockProcessInspector {
        display_names: Mutex::new(names),
        call_count: Mutex::new(0),
    });

    let mut config = empty_config();
    config.applications.push(ApplicationConfiguration::new(
        1,
        "C:\\apps\\code.exe".to_string(),
        CycleMode::NextApp,
        false,
    ));

    let sut = AppRegistryCache::new(db, win_enum, pkg_apps, inspector);
    sut.prepopulate(&config);

    assert_eq!(sut.get_display_name("code.exe", None), "Visual Studio Code");
}
