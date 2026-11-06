use super::{application_window::ApplicationWindow, process_inspector::ProcessInspector};
use crate::configuration::Configuration;
use std::{collections::HashMap, fs, path::PathBuf};

/// Persistent friendly-name cache used by dynamic mode and statistics.
#[derive(Default)]
pub struct AppRegistryCache {
    path: Option<PathBuf>,
    entries: HashMap<String, String>,
}

impl AppRegistryCache {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let entries = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .or_else(|| {
                path.parent()
                    .and_then(|directory| {
                        crate::legacy_import::profile_bundle(directory)
                            .ok()
                            .flatten()
                    })
                    .map(|bundle| {
                        bundle
                            .app_registry
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect()
                    })
            })
            .unwrap_or_default();
        Self {
            path: Some(path),
            entries,
        }
    }

    pub fn prepopulate(&mut self, config: &Configuration, windows: &[ApplicationWindow]) {
        for window in windows {
            let _ = self.get_display_name(window.process_name(), Some(&window.process_image_path));
        }
        for app in &config.applications {
            let _ = self.get_display_name(app.process_name(), Some(&app.process_path));
        }
    }

    pub fn get_display_name(&mut self, process_name: &str, process_path: Option<&str>) -> String {
        let key = process_name.to_ascii_lowercase();
        if let Some(value) = self.entries.get(&key) {
            return value.clone();
        }
        let value = process_path
            .map(ProcessInspector::display_name)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| process_name.trim_end_matches(".exe").to_string());
        self.entries.insert(key, value.clone());
        self.flush();
        value
    }

    fn flush(&self) {
        let Some(path) = &self.path else { return };
        if let Ok(text) = serde_json::to_string_pretty(&self.entries)
            && let Err(error) = crate::atomic_file::write(path, text.as_bytes())
        {
            crate::logging::debug(format!("Failed to save app registry: {error}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_friendly_names() {
        let path =
            std::env::temp_dir().join(format!("omnispot-registry-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut first = AppRegistryCache::new(&path);
        assert_eq!(
            first.get_display_name("editor.exe", Some(r"C:\Apps\Editor.exe")),
            "Editor"
        );
        let second = AppRegistryCache::new(&path);
        assert_eq!(
            second.entries.get("editor.exe").map(String::as_str),
            Some("Editor")
        );
        let _ = fs::remove_file(path);
    }
}
