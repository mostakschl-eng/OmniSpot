use super::window_title_parser::WindowTitleParser;
use crate::ui::view_models::application_shortcut::{ApplicationShortcut, CycleMode};
use crate::ui::view_models::overlay::{OverlayRow, OverlayState, OverlayWindowGroup};
use std::collections::HashSet;

pub(crate) struct DiscoveredWindow {
    pub process_name: String,
    pub process_path: String,
    pub title: String,
    pub handle: isize,
    pub needs_elevation: bool,
}

#[derive(Default)]
pub(crate) struct AppOverlayService {
    pub is_visible: bool,
}

impl AppOverlayService {
    pub(crate) fn show(
        &mut self,
        applications: &[ApplicationShortcut],
        windows: &[DiscoveredWindow],
        dynamic_apps: &[ApplicationShortcut],
        dynamic_mode_enabled: bool,
        current_handle: Option<isize>,
    ) -> OverlayState {
        let mut state = OverlayState::default();
        let mut grouped_processes = HashSet::new();
        for app in applications {
            let process_name = app
                .process_path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&app.process_path);
            let matching = windows
                .iter()
                .filter(|window| window.process_name.eq_ignore_ascii_case(process_name))
                .collect::<Vec<_>>();
            let active = matching
                .iter()
                .any(|window| Some(window.handle) == current_handle);
            let row = OverlayRow {
                name: app.display_name.clone(),
                key: app.key.clone(),
                active,
                elevated: matching.iter().any(|window| window.needs_elevation),
            };
            if matching.is_empty() {
                state.launchable.push(row);
            } else {
                state.running.push(row);
            }

            if app.cycle_mode == CycleMode::NextWindow
                && grouped_processes.insert(process_name.to_ascii_lowercase())
                && matching.len() >= 2
            {
                let mut matching = matching;
                matching.sort_by_key(|window| window.handle);
                matching.truncate(10);
                let titles = matching
                    .iter()
                    .map(|window| window.title.clone())
                    .collect::<Vec<_>>();
                let suffix = WindowTitleParser::find_common_suffix(&titles);
                let windows = matching
                    .iter()
                    .enumerate()
                    .map(|(index, window)| OverlayRow {
                        name: WindowTitleParser::strip_suffix(&window.title, suffix).to_owned(),
                        key: if index == 9 {
                            "0".to_owned()
                        } else {
                            (index + 1).to_string()
                        },
                        active: Some(window.handle) == current_handle,
                        elevated: window.needs_elevation,
                    })
                    .collect();
                state.window_groups.push(OverlayWindowGroup {
                    app_name: app.display_name.clone(),
                    windows,
                });
            }
        }
        if dynamic_mode_enabled {
            state.dynamic = dynamic_apps
                .iter()
                .map(|app| OverlayRow {
                    name: app.display_name.clone(),
                    key: app.key.clone(),
                    active: windows.iter().any(|window| {
                        window.process_path.eq_ignore_ascii_case(&app.process_path)
                            && Some(window.handle) == current_handle
                    }),
                    elevated: windows.iter().any(|window| {
                        window.process_path.eq_ignore_ascii_case(&app.process_path)
                            && window.needs_elevation
                    }),
                })
                .collect();
        }
        self.is_visible = true;
        state
    }

    pub(crate) fn hide(&mut self) {
        self.is_visible = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{AppOverlayService, DiscoveredWindow};
    use crate::ui::view_models::application_shortcut::{ApplicationShortcut, CycleMode};

    #[test]
    fn groups_running_windows_and_numbers_them() {
        let app = ApplicationShortcut {
            key: "V".into(),
            display_name: "Vivaldi".into(),
            process_path: r"C:\Apps\vivaldi.exe".into(),
            cycle_mode: CycleMode::NextWindow,
            start_if_not_running: false,
        };
        let windows = [
            DiscoveredWindow {
                process_name: "vivaldi.exe".into(),
                process_path: app.process_path.clone(),
                title: "Tab One - Vivaldi".into(),
                handle: 2,
                needs_elevation: false,
            },
            DiscoveredWindow {
                process_name: "vivaldi.exe".into(),
                process_path: app.process_path.clone(),
                title: "Tab Two - Vivaldi".into(),
                handle: 1,
                needs_elevation: false,
            },
        ];
        let mut service = AppOverlayService::default();
        let state = service.show(&[app], &windows, &[], false, Some(2));
        assert_eq!(state.running.len(), 1);
        assert_eq!(state.window_groups[0].windows[0].name, "Tab Two");
        assert_eq!(state.window_groups[0].windows[0].key, "1");
        assert!(state.window_groups[0].windows[1].active);
        service.hide();
        assert!(!service.is_visible);
    }
}
