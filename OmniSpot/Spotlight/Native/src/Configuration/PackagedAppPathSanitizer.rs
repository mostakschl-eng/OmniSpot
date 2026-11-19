use super::application_type::ApplicationType;
use super::configuration::{ApplicationConfiguration, Configuration};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PackagedAppInfo {
    pub aumid: String,
    pub icon_path: Option<String>,
}

pub(crate) trait IPackagedAppsService {
    fn get_by_package_family_name(&self, family_name: &str) -> Option<PackagedAppInfo>;
    fn get_installed_path_by_aumid(&self, aumid: &str) -> Option<String>;
    fn get_by_installed_path(&self, process_path: &str) -> Option<PackagedAppInfo> {
        let _ = process_path;
        None
    }
}

impl<T: IPackagedAppsService + ?Sized> IPackagedAppsService for &T {
    fn get_by_package_family_name(&self, family_name: &str) -> Option<PackagedAppInfo> {
        (**self).get_by_package_family_name(family_name)
    }
    fn get_installed_path_by_aumid(&self, aumid: &str) -> Option<String> {
        (**self).get_installed_path_by_aumid(aumid)
    }
    fn get_by_installed_path(&self, process_path: &str) -> Option<PackagedAppInfo> {
        (**self).get_by_installed_path(process_path)
    }
}

impl<T: IPackagedAppsService + ?Sized> IPackagedAppsService for std::sync::Arc<T> {
    fn get_by_package_family_name(&self, family_name: &str) -> Option<PackagedAppInfo> {
        (**self).get_by_package_family_name(family_name)
    }
    fn get_installed_path_by_aumid(&self, aumid: &str) -> Option<String> {
        (**self).get_installed_path_by_aumid(aumid)
    }
    fn get_by_installed_path(&self, process_path: &str) -> Option<PackagedAppInfo> {
        (**self).get_by_installed_path(process_path)
    }
}

pub(crate) fn parse_package_family_name_from_dir(dir_name: &str) -> Option<String> {
    let parts: Vec<&str> = dir_name.split('_').collect();
    if parts.len() < 5 {
        return None;
    }
    let name = parts[0];
    let publisher_id = *parts.last()?;

    if name.len() < 3 || name.len() > 50 {
        return None;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return None;
    }
    if publisher_id.len() != 13
        || !publisher_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return None;
    }

    let version_parts: Vec<&str> = parts[1].split('.').collect();
    if version_parts.len() != 4
        || !version_parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }

    Some(format!("{}_{}", name, publisher_id))
}

pub(crate) fn get_package_family_name(process_path: &str) -> Option<String> {
    let mut current = Path::new(process_path).parent();
    while let Some(dir) = current {
        if let Some(file_name) = dir.file_name().and_then(|f| f.to_str()) {
            if let Some(family) = parse_package_family_name_from_dir(file_name) {
                return Some(family);
            }
        }
        current = dir.parent();
    }
    None
}

pub(crate) fn get_path_relative_to_package(
    stale_path: &str,
    new_installed_path: &str,
) -> Option<String> {
    let new_path = Path::new(new_installed_path);
    let package_root = new_path.parent()?;
    let package_root_str = package_root.to_str()?;

    let stale_str = stale_path;
    if stale_str.len() > package_root_str.len()
        && stale_str[..package_root_str.len()].eq_ignore_ascii_case(package_root_str)
    {
        let sep = stale_str.as_bytes()[package_root_str.len()];
        if sep == b'\\' || sep == b'/' {
            let remainder = &stale_str[package_root_str.len() + 1..];
            if let Some(pos) = remainder.find(['\\', '/']) {
                return Some(remainder[pos + 1..].to_string());
            }
        }
    }

    None
}

pub(crate) struct PackagedAppPathSanitizer<P: IPackagedAppsService> {
    packaged_apps_service: P,
}

impl<P: IPackagedAppsService> PackagedAppPathSanitizer<P> {
    pub(crate) fn new(packaged_apps_service: P) -> Self {
        Self {
            packaged_apps_service,
        }
    }

    pub(crate) fn sanitize(&self, config: &Configuration) -> Option<Configuration> {
        let mut updated_apps = Vec::with_capacity(config.applications.len());
        let mut changed = false;

        for original in &config.applications {
            let mut app = if original.application_type == ApplicationType::Win32 {
                self.try_convert_to_packaged(original)
                    .unwrap_or_else(|| original.clone())
            } else {
                original.clone()
            };

            if let Some(fixed) = self.try_fix_stale_path(&app) {
                app = fixed;
            }

            if app != *original {
                changed = true;
            }
            updated_apps.push(app);
        }

        if !changed {
            return None;
        }

        let mut new_config = config.clone();
        new_config.applications = updated_apps;
        Some(new_config)
    }

    fn try_convert_to_packaged(
        &self,
        app: &ApplicationConfiguration,
    ) -> Option<ApplicationConfiguration> {
        let package_family_name = get_package_family_name(&app.process_path)?;
        let packaged_app = self
            .packaged_apps_service
            .get_by_package_family_name(&package_family_name)?;

        let mut updated = app.clone();
        updated.application_type = ApplicationType::Packaged;
        updated.aumid = Some(packaged_app.aumid);
        Some(updated)
    }

    fn try_fix_stale_path(
        &self,
        app: &ApplicationConfiguration,
    ) -> Option<ApplicationConfiguration> {
        if app.application_type != ApplicationType::Packaged
            || Path::new(&app.process_path).is_file()
        {
            return None;
        }

        let aumid = app.aumid.as_deref()?;
        let new_installed_path = self
            .packaged_apps_service
            .get_installed_path_by_aumid(aumid)?;

        let exe_file_name = Path::new(&app.process_path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(&app.process_path);

        let exe_relative_path =
            get_path_relative_to_package(&app.process_path, &new_installed_path)
                .unwrap_or_else(|| exe_file_name.to_string());

        let new_process_path = Path::new(&new_installed_path)
            .join(exe_relative_path)
            .to_str()
            .unwrap_or(&app.process_path)
            .to_string();

        let mut updated = app.clone();
        updated.process_path = new_process_path;
        Some(updated)
    }
}

#[cfg(test)]
#[path = "../tests/Configuration/PackagedAppPathSanitizerTests.rs"]
mod tests;
