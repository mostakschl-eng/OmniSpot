use super::application_window::ApplicationWindow;
use super::file_name;
use super::installed_packages::InstalledPackages;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RunningApplicationInfo {
    pub process_id: u32,
    pub process_name: String,
    pub process_image_path: String,
    pub is_packaged_app: bool,
}

pub(super) fn get_running_applications(
    windows: &[ApplicationWindow],
    excluded: &[String],
    packages: &[String],
) -> Vec<RunningApplicationInfo> {
    let packages = InstalledPackages::new(packages);
    let excluded: HashSet<_> = excluded
        .iter()
        .map(|p| lower_invariant(file_name(p)))
        .chain(Some("omnispot.exe".to_string()))
        .collect();
    let mut seen = HashSet::new();
    let mut result = Vec::<RunningApplicationInfo>::new();
    for window in windows {
        let name = file_name(&window.process_image_path);
        let key = lower_invariant(name);
        if !seen.insert(key.clone()) || excluded.contains(&key) {
            continue;
        }
        result.push(RunningApplicationInfo {
            process_id: window.process_id,
            process_name: name.to_string(),
            process_image_path: window.process_image_path.clone(),
            is_packaged_app: packages.contains(&window.process_image_path),
        });
    }
    result.sort_by_cached_key(|app| super::ordinal_key(&app.process_name));
    result
}

fn lower_invariant(value: &str) -> String {
    value.chars().map(super::lower_invariant).collect()
}
