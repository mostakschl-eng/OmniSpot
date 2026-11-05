use crate::{
    configuration::{ApplicationConfiguration, ApplicationType, CycleMode, Key},
    window_discovery::{AppRegistryCache, ApplicationWindow, process_inspector::ProcessInspector},
};
use std::collections::HashSet;

pub struct DynamicModeService;
impl DynamicModeService {
    pub fn get_apps_for_key(
        letter: Key,
        static_apps: &[ApplicationConfiguration],
        windows: &[ApplicationWindow],
    ) -> Vec<ApplicationConfiguration> {
        Self::get_apps_for_key_impl(letter, static_apps, windows, |window| {
            ProcessInspector::display_name(&window.process_image_path)
        })
    }

    pub fn get_apps_for_key_with_registry(
        letter: Key,
        static_apps: &[ApplicationConfiguration],
        windows: &[ApplicationWindow],
        registry: &mut AppRegistryCache,
    ) -> Vec<ApplicationConfiguration> {
        Self::get_apps_for_key_impl(letter, static_apps, windows, |window| {
            registry.get_display_name(window.process_name(), Some(&window.process_image_path))
        })
    }

    fn get_apps_for_key_impl(
        letter: Key,
        static_apps: &[ApplicationConfiguration],
        windows: &[ApplicationWindow],
        mut display_name: impl FnMut(&ApplicationWindow) -> String,
    ) -> Vec<ApplicationConfiguration> {
        if static_apps.iter().any(|app| app.key == letter) {
            return Vec::new();
        }
        let excluded = static_apps
            .iter()
            .map(|app| app.process_name().to_ascii_lowercase())
            .chain(["omnispot.exe".to_string()])
            .collect::<HashSet<_>>();
        windows
            .iter()
            .filter(|window| !excluded.contains(&window.process_name().to_ascii_lowercase()))
            .filter(|window| dynamic_key(&display_name(window)) == Some(letter))
            .map(|window| ApplicationConfiguration {
                key: letter,
                process_path: window.process_image_path.clone(),
                cycle_mode: CycleMode::NextApp,
                start_if_not_running: false,
                application_type: ApplicationType::Win32,
                aumid: None,
            })
            .fold(Vec::new(), |mut apps, app| {
                if !apps.iter().any(|existing: &ApplicationConfiguration| {
                    existing
                        .process_path
                        .eq_ignore_ascii_case(&app.process_path)
                }) {
                    apps.push(app);
                }
                apps
            })
    }
}

fn dynamic_key(process_name: &str) -> Option<Key> {
    let character = process_name
        .chars()
        .find(|character| character.is_ascii_alphabetic())?
        .to_ascii_uppercase();
    Some(match character {
        'A' => Key::A,
        'B' => Key::B,
        'C' => Key::C,
        'D' => Key::D,
        'E' => Key::E,
        'F' => Key::F,
        'G' => Key::G,
        'H' => Key::H,
        'I' => Key::I,
        'J' => Key::J,
        'K' => Key::K,
        'L' => Key::L,
        'M' => Key::M,
        'N' => Key::N,
        'O' => Key::O,
        'P' => Key::P,
        'Q' => Key::Q,
        'R' => Key::R,
        'S' => Key::S,
        'T' => Key::T,
        'U' => Key::U,
        'V' => Key::V,
        'W' => Key::W,
        'X' => Key::X,
        'Y' => Key::Y,
        'Z' => Key::Z,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn window(path: &str) -> ApplicationWindow {
        ApplicationWindow {
            handle: 1,
            title: "Test".to_string(),
            process_id: 1,
            process_image_path: path.to_string(),
            is_minimized: false,
            is_cloaked: false,
            extended_style: 0,
            style: 0,
            position: (0, 0),
            size: (100, 100),
            needs_elevation: false,
        }
    }
    #[test]
    fn assigns_first_process_letter_when_not_static() {
        let apps =
            DynamicModeService::get_apps_for_key(Key::N, &[], &[window(r"C:\Windows\notepad.exe")]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].key, Key::N);
    }
    #[test]
    fn excludes_static_processes() {
        let static_app = ApplicationConfiguration {
            key: Key::N,
            process_path: r"C:\Windows\notepad.exe".to_string(),
            cycle_mode: CycleMode::NextApp,
            start_if_not_running: false,
            application_type: ApplicationType::Win32,
            aumid: None,
        };
        assert!(
            DynamicModeService::get_apps_for_key(
                Key::N,
                &[static_app],
                &[window(r"C:\Windows\notepad.exe")]
            )
            .is_empty()
        );
    }
}
