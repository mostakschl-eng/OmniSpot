use super::*;
use crate::window_discovery::window_enumerator::FocusedAppWindows;

#[derive(Default)]
struct Windows {
    windows: Vec<ApplicationWindow>,
    current: Option<ApplicationWindow>,
}

impl IWindowEnumerator for Windows {
    fn get_windows(&self) -> Vec<ApplicationWindow> {
        self.windows.clone()
    }
    fn get_cached_windows(&mut self) -> Vec<ApplicationWindow> {
        self.windows.clone()
    }
    fn get_current_window(&self) -> Option<ApplicationWindow> {
        self.current.clone()
    }
    fn get_focused_app_windows(
        &self,
        all: &[ApplicationWindow],
        applications: &[ApplicationConfiguration],
    ) -> FocusedAppWindows {
        window_discovery::window_enumerator::get_focused_app_windows(
            self.current.clone(),
            all.to_vec(),
            applications,
        )
    }
    fn get_all_windows(&self) -> Vec<ApplicationWindow> {
        self.windows.clone()
    }
}

fn window(handle: i64) -> ApplicationWindow {
    ApplicationWindow {
        handle,
        process_id: 1,
        process_image_path: r"C:\Apps\App.exe".into(),
        ..Default::default()
    }
}

#[test]
fn next_window_cycle_keeps_original_history_when_z_order_changes() {
    let mut switcher = Switcher::new(Windows::default());
    let all = vec![window(3), window(2), window(1)];
    for (current, expected) in [(2, 3), (3, 1), (1, 2)] {
        assert_eq!(
            switcher
                .get_next_window(&all, &window(current))
                .unwrap()
                .handle,
            expected
        );
    }
    let reordered = vec![window(1), window(3), window(2)];
    assert_eq!(
        switcher
            .get_next_window(&reordered, &window(2))
            .unwrap()
            .handle,
        3
    );
    assert_eq!(switcher.next_windows, vec![2, 3, 1]);
}

#[test]
fn newly_discovered_windows_are_visited_before_repeating_the_cycle() {
    let mut switcher = Switcher::new(Windows::default());
    switcher.next_windows = vec![1, 2];
    assert_eq!(
        switcher
            .get_next_window(&[window(3), window(2), window(1)], &window(2))
            .unwrap()
            .handle,
        3
    );
}

#[test]
fn missing_app_does_not_start_when_starting_is_disabled() {
    let mut switcher = Switcher::new(Windows::default());
    let app = ApplicationConfiguration::new(0, "missing.exe".into(), CycleMode::NextApp, false);
    assert!(switcher.execute(&[app], "en-US").unwrap().is_none());
}

#[test]
fn invalid_digit_and_already_focused_digit_keep_original_result_contract() {
    let current = window(1);
    let switcher = Switcher::new(Windows {
        windows: vec![current.clone()],
        current: Some(current),
    });
    let app = ApplicationConfiguration::new(0, "App.exe".into(), CycleMode::NextWindow, false);
    for index in [-1, 1] {
        let (handled, focused) = switcher
            .switch_to_window_by_index(std::slice::from_ref(&app), index)
            .unwrap();
        assert!(!handled);
        assert!(focused.is_none());
    }
    let (handled, focused) = switcher.switch_to_window_by_index(&[app], 0).unwrap();
    assert!(handled);
    assert!(focused.is_none());
}

#[test]
fn switch_result_factories_preserve_original_show_state_and_cached_name() {
    let mut original = window(10);
    original.state = SW_SHOWMINIMIZED.0 as u32;
    original.needs_elevation = true;
    let switched = AppSwitchResult::from_application_window(&original);
    assert_eq!(switched.state, SW_SHOW.0 as u32);
    assert!(switched.needs_elevation);
    assert!(!switched.was_started);
    let started = AppSwitchResult::from_started(r"C:\Apps\Other.exe".into());
    assert!(started.was_started);
    assert!(started.process_id.is_none());
    assert_eq!(started.process_name, "Other.exe");
}

#[test]
fn process_matching_preserves_turkish_case_rules() {
    assert!(culture_ends_with("testI.exe", "ı.exe", "tr-TR"));
    assert!(!culture_ends_with("testI.exe", "i.exe", "tr-TR"));
}
