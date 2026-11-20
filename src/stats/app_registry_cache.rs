use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::path::Path;
use crate::configuration::configuration::Configuration;
use crate::window_discovery::installed_packages::InstalledPackages;
use crate::window_discovery::i_window_enumerator::IWindowEnumerator;
use super::i_app_registry_cache::IAppRegistryCache;
use super::storage::StatsDatabase;

pub(crate) trait IPackagedAppsProvider: Send + Sync {
    fn get_installed_packages(&self) -> Vec<String>;
    fn get_display_name(&self, path: &str) -> Option<String>;
}

pub(crate) trait IProcessInspectorProvider: Send + Sync {
    fn get_process_display_name(&self, path: &str) -> String;
}

#[derive(Default)]
pub(crate) struct DefaultPackagedAppsProvider;

impl IPackagedAppsProvider for DefaultPackagedAppsProvider {
    fn get_installed_packages(&self) -> Vec<String> {
        let svc = crate::window_discovery::packaged_apps_service::PackagedAppsService::default();
        svc.get_installed_packages().unwrap_or_default()
    }

    fn get_display_name(&self, path: &str) -> Option<String> {
        let svc = crate::window_discovery::packaged_apps_service::PackagedAppsService::default();
        svc.get_display_name(path).ok().flatten()
    }
}

#[derive(Default)]
pub(crate) struct DefaultProcessInspectorProvider;

impl IProcessInspectorProvider for DefaultProcessInspectorProvider {
    fn get_process_display_name(&self, path: &str) -> String {
        crate::window_discovery::process_inspector::get_process_display_name(path)
    }
}

pub(crate) struct AppRegistryCache {
    cache: RwLock<HashMap<String, String>>,
    db: Arc<Mutex<StatsDatabase>>,
    window_enumerator: Arc<dyn IWindowEnumerator + Send + Sync>,
    packaged_apps: Arc<dyn IPackagedAppsProvider>,
    process_inspector: Arc<dyn IProcessInspectorProvider>,
}

impl AppRegistryCache {
    pub(crate) fn new(
        db: Arc<Mutex<StatsDatabase>>,
        window_enumerator: Arc<dyn IWindowEnumerator + Send + Sync>,
        packaged_apps: Arc<dyn IPackagedAppsProvider>,
        process_inspector: Arc<dyn IProcessInspectorProvider>,
    ) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            db,
            window_enumerator,
            packaged_apps,
            process_inspector,
        }
    }

    pub(crate) fn prepopulate(&self, config: &Configuration) {
        let windows = self.window_enumerator.get_windows();

        // 1. From database first
        if let Ok(db) = self.db.lock() {
            let mut cache = self.cache.write().unwrap();
            for (proc_name, disp_name) in &db.app_registry {
                cache.insert(proc_name.to_lowercase(), disp_name.clone());
            }
        }

        let installed_paths = self.packaged_apps.get_installed_packages();
        let installed_packages = InstalledPackages::new(&installed_paths);

        // 2. Running windows
        for window in windows {
            let proc_name = window.process_name();
            let exists = {
                let cache = self.cache.read().unwrap();
                cache.contains_key(&proc_name.to_lowercase())
            };
            if !exists {
                self.add(proc_name, &window.process_image_path, &installed_packages);
            }
        }

        // 3. Configured apps
        for app in &config.applications {
            let proc_name = app.process_name();
            let exists = {
                let cache = self.cache.read().unwrap();
                cache.contains_key(&proc_name.to_lowercase())
            };
            if !exists {
                self.add(proc_name, &app.process_path, &installed_packages);
            }
        }
    }

    pub(crate) fn try_add(&self, process_name: &str, process_path: &str) -> bool {
        let key = process_name.to_lowercase();
        {
            let cache = self.cache.read().unwrap();
            if cache.contains_key(&key) {
                return false;
            }
        }

        let installed_paths = self.packaged_apps.get_installed_packages();
        let installed_packages = InstalledPackages::new(&installed_paths);
        self.add(process_name, process_path, &installed_packages);
        true
    }

    pub(crate) fn get_display_name(&self, process_name: &str, process_path: Option<&str>) -> String {
        let key = process_name.to_lowercase();
        {
            let cache = self.cache.read().unwrap();
            if let Some(cached) = cache.get(&key) {
                return cached.clone();
            }
        }

        if let Some(path) = process_path {
            if !path.is_empty() && self.try_add(process_name, path) {
                let cache = self.cache.read().unwrap();
                if let Some(cached) = cache.get(&key) {
                    return cached.clone();
                }
            }
        }

        file_stem_without_ext(process_name)
    }

    fn add(&self, process_name: &str, process_path: &str, installed_packages: &InstalledPackages) {
        let is_packaged = installed_packages.contains(process_path);
        let mut display_name = if is_packaged {
            self.packaged_apps.get_display_name(process_path)
        } else {
            None
        };

        if display_name.as_ref().map_or(true, |s| s.is_empty()) {
            display_name = Some(self.process_inspector.get_process_display_name(process_path));
        }

        let name = display_name.unwrap_or_else(|| file_stem_without_ext(process_name));
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(process_name.to_lowercase(), name.clone());
        }

        self.persist_to_registry(process_name, &name);
    }

    fn persist_to_registry(&self, process_name: &str, display_name: &str) {
        if let Ok(mut db) = self.db.lock() {
            db.app_registry
                .entry(process_name.to_string())
                .or_insert_with(|| display_name.to_string());
        }
    }
}

impl IAppRegistryCache for AppRegistryCache {
    fn get_display_name(&self, process_name: &str, process_path: Option<&str>) -> String {
        self.get_display_name(process_name, process_path)
    }
}

impl IAppRegistryCache for Arc<AppRegistryCache> {
    fn get_display_name(&self, process_name: &str, process_path: Option<&str>) -> String {
        self.as_ref().get_display_name(process_name, process_path)
    }
}

fn file_stem_without_ext(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

#[cfg(test)]
#[path = "../tests/stats/app_registry_cache_tests.rs"]
mod tests;
