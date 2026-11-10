use crate::configuration::application_type::ApplicationType;
use crate::configuration::configuration::{ApplicationConfiguration, CycleMode};
use crate::runtime::file_name;
use crate::window_discovery::application_window::ApplicationWindow;
use crate::window_discovery::i_app_name_resolver::IAppNameResolver;
use crate::window_discovery::i_window_enumerator::IWindowEnumerator;
use crate::window_discovery::installed_packages::InstalledPackages;
use crate::window_discovery::ordinal_key;
use crate::window_discovery::packaged_apps_service::PackagedAppsService;
use std::collections::HashSet;

pub(crate) struct DynamicModeService<E: IWindowEnumerator, R: IAppNameResolver> {
    pub window_enumerator: E,
    pub app_name_resolver: R,
    pub packaged_apps_service: PackagedAppsService,
}

impl<E: IWindowEnumerator, R: IAppNameResolver> DynamicModeService<E, R> {
    pub fn get_all_dynamic_apps(
        &self,
        static_apps: &[ApplicationConfiguration],
        windows: &[ApplicationWindow],
    ) -> windows::core::Result<Vec<ApplicationConfiguration>> {
        let installed =
            InstalledPackages::new(&self.packaged_apps_service.get_installed_packages()?);
        Ok(self.build_dynamic_apps(static_apps, windows, &installed))
    }

    fn build_dynamic_apps(
        &self,
        static_apps: &[ApplicationConfiguration],
        windows: &[ApplicationWindow],
        installed: &InstalledPackages,
    ) -> Vec<ApplicationConfiguration> {
        let static_keys: HashSet<_> = static_apps.iter().map(|app| app.key).collect();
        let excluded = get_excluded_process_names(static_apps);
        let mut paths = HashSet::new();
        windows
            .iter()
            .filter(|window| paths.insert(ordinal_key(&window.process_image_path)))
            .filter(|window| {
                !excluded.contains(&ordinal_key(file_name(&window.process_image_path)))
            })
            .filter_map(|window| {
                let key = self.app_name_resolver.get_dynamic_key(
                    file_name(&window.process_image_path),
                    &window.process_image_path,
                )?;
                if static_keys.contains(&key) {
                    return None;
                }
                let mut app = ApplicationConfiguration::new(
                    key,
                    window.process_image_path.clone(),
                    CycleMode::NextApp,
                    false,
                );
                app.application_type = if installed.contains(&window.process_image_path) {
                    ApplicationType::Packaged
                } else {
                    ApplicationType::Win32
                };
                Some(app)
            })
            .collect()
    }

    pub fn get_apps_for_key(
        &mut self,
        letter: u32,
        static_apps: &[ApplicationConfiguration],
    ) -> Vec<ApplicationConfiguration> {
        if static_apps.iter().any(|app| app.key == letter) {
            return Vec::new();
        }
        let windows = self.window_enumerator.get_cached_windows();
        let excluded = get_excluded_process_names(static_apps);
        let mut paths = HashSet::new();
        windows
            .iter()
            .filter(|window| {
                !excluded.contains(&ordinal_key(file_name(&window.process_image_path)))
            })
            .filter(|window| {
                self.app_name_resolver.get_dynamic_key(
                    file_name(&window.process_image_path),
                    &window.process_image_path,
                ) == Some(letter)
            })
            .filter(|window| paths.insert(ordinal_key(&window.process_image_path)))
            .map(|window| {
                ApplicationConfiguration::new(
                    letter,
                    window.process_image_path.clone(),
                    CycleMode::NextApp,
                    false,
                )
            })
            .collect()
    }
}

fn get_excluded_process_names(static_apps: &[ApplicationConfiguration]) -> HashSet<Vec<u16>> {
    static_apps
        .iter()
        .map(|app| app.process_name())
        .chain(["omnispot.exe"])
        .map(ordinal_key)
        .collect()
}

#[cfg(test)]
#[path = "../tests/Input/DynamicModeServiceTests.rs"]
mod tests;
