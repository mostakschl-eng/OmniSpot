use super::*;
use crate::input::keyboard_input::A;
use crate::window_discovery::window_enumerator::FocusedAppWindows;
use std::cell::Cell;

#[derive(Default)]
struct Windows {
    windows: Vec<ApplicationWindow>,
    reads: usize,
}

impl IWindowEnumerator for Windows {
    fn get_windows(&self) -> Vec<ApplicationWindow> {
        self.windows.clone()
    }
    fn get_cached_windows(&mut self) -> Vec<ApplicationWindow> {
        self.reads += 1;
        self.windows.clone()
    }
    fn get_current_window(&self) -> Option<ApplicationWindow> {
        None
    }
    fn get_focused_app_windows(
        &self,
        _: &[ApplicationWindow],
        _: &[ApplicationConfiguration],
    ) -> FocusedAppWindows {
        FocusedAppWindows {
            focused_window: None,
            all_windows: Vec::new(),
        }
    }
    fn get_all_windows(&self) -> Vec<ApplicationWindow> {
        self.windows.clone()
    }
}

#[derive(Default)]
struct Resolver {
    calls: Cell<usize>,
}
impl IAppNameResolver for Resolver {
    fn get_dynamic_key(&self, name: &str, _: &str) -> Option<u32> {
        self.calls.set(self.calls.get() + 1);
        if name.starts_with('1') {
            return None;
        }
        // Different keys on duplicate paths expose the pipeline ordering.
        Some(A + if self.calls.get() == 1 { 1 } else { 0 })
    }
}

fn window(path: &str) -> ApplicationWindow {
    ApplicationWindow {
        process_image_path: path.into(),
        ..Default::default()
    }
}

fn service(windows: Vec<ApplicationWindow>) -> DynamicModeService<Windows, Resolver> {
    DynamicModeService {
        window_enumerator: Windows { windows, reads: 0 },
        app_name_resolver: Resolver::default(),
        packaged_apps_service: PackagedAppsService::default(),
    }
}

#[test]
fn statically_assigned_key_returns_without_reading_windows_or_resolving_names() {
    let mut service = service(vec![window(r"C:\a.exe")]);
    let apps = [ApplicationConfiguration::new(
        A,
        "other.exe".into(),
        CycleMode::Hide,
        true,
    )];
    assert!(service.get_apps_for_key(A, &apps).is_empty());
    assert_eq!(service.window_enumerator.reads, 0);
    assert_eq!(service.app_name_resolver.calls.get(), 0);
}

#[test]
fn get_apps_for_key_filters_before_distinct_and_keeps_default_win32_type() {
    let mut service = service(vec![
        window(r"C:\a.exe"),
        window(r"c:\A.EXE"),
        window(r"C:\a.exe"),
        window(r"C:\OmniSpot.exe"),
        window(r"C:\1.exe"),
    ]);
    let apps = service.get_apps_for_key(A, &[]);
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].process_path, r"c:\A.EXE");
    assert_eq!(apps[0].key, A);
    assert_eq!(apps[0].cycle_mode, CycleMode::NextApp);
    assert!(!apps[0].start_if_not_running);
    assert_eq!(apps[0].application_type, ApplicationType::Win32);
    assert_eq!(service.app_name_resolver.calls.get(), 4);
}

#[test]
fn get_all_dynamic_apps_deduplicates_before_resolving_and_marks_packages() {
    let service = service(Vec::new());
    let windows = [
        window(r"C:\Apps\Package_1\a.exe"),
        window(r"c:\apps\PACKAGE_1\A.EXE"),
        window(r"D:\b.exe"),
        window(r"C:\OmniSpot.exe"),
        window(r"C:\1.exe"),
    ];
    let installed = InstalledPackages::new(&[r"C:\Apps\Package_1".into()]);
    let apps = service.build_dynamic_apps(&[], &windows, &installed);
    assert_eq!(apps.len(), 2);
    assert_eq!(apps[0].key, A + 1);
    assert_eq!(apps[0].application_type, ApplicationType::Packaged);
    assert_eq!(apps[1].application_type, ApplicationType::Win32);
    assert_eq!(service.app_name_resolver.calls.get(), 3);
}

#[test]
fn static_process_names_and_assigned_keys_are_excluded_independently() {
    let service = service(Vec::new());
    let apps = [ApplicationConfiguration::new(
        A + 1,
        r"D:\static.exe".into(),
        CycleMode::NextApp,
        false,
    )];
    let windows = [
        window(r"C:\STATIC.EXE"),
        window(r"C:\first.exe"),
        window(r"C:\second.exe"),
    ];
    let result = service.build_dynamic_apps(&apps, &windows, &InstalledPackages::new(&[]));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].process_path, r"C:\second.exe");
}
