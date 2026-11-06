use super::model::{ApplicationConfiguration, ApplicationType, Configuration};
use std::path::{Path, PathBuf};

pub trait PackagedAppResolver {
    fn package_family_for_path(&self, path: &Path) -> Option<String>;
    fn aumid_for_family(&self, family: &str) -> Option<String>;
    fn installed_path_for_aumid(&self, aumid: &str) -> Option<PathBuf>;
}

impl PackagedAppResolver for Vec<super::super::window_discovery::packaged_apps::PackagedApp> {
    fn package_family_for_path(&self, path: &Path) -> Option<String> {
        self.iter()
            .find(|package| path.starts_with(Path::new(&package.installed_path)))?
            .aumid
            .as_deref()?
            .split_once('!')
            .map(|(family, _)| family.to_string())
    }

    fn aumid_for_family(&self, family: &str) -> Option<String> {
        self.iter()
            .filter_map(|package| package.aumid.as_ref())
            .find(|aumid| {
                aumid
                    .split_once('!')
                    .is_some_and(|(candidate, _)| candidate.eq_ignore_ascii_case(family))
            })
            .cloned()
    }

    fn installed_path_for_aumid(&self, aumid: &str) -> Option<PathBuf> {
        self.iter()
            .find(|package| {
                package
                    .aumid
                    .as_deref()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(aumid))
            })
            .map(|package| PathBuf::from(&package.installed_path))
    }
}
pub struct PackagedAppPathSanitizer<R> {
    resolver: R,
}
impl<R: PackagedAppResolver> PackagedAppPathSanitizer<R> {
    pub fn new(resolver: R) -> Self {
        Self { resolver }
    }
    pub fn sanitize(&self, config: &Configuration) -> Option<Configuration> {
        let apps: Vec<_> = config
            .applications
            .iter()
            .map(|app| self.sanitize_app(app))
            .collect();
        (apps != config.applications).then(|| Configuration {
            applications: apps,
            ..config.clone()
        })
    }
    fn sanitize_app(&self, app: &ApplicationConfiguration) -> ApplicationConfiguration {
        let mut updated = app.clone();
        if app.application_type == ApplicationType::Win32
            && let Some(family) = self
                .resolver
                .package_family_for_path(Path::new(&app.process_path))
            && let Some(aumid) = self.resolver.aumid_for_family(&family)
        {
            updated.application_type = ApplicationType::Packaged;
            updated.aumid = Some(aumid);
        }
        if updated.application_type == ApplicationType::Packaged
            && !Path::new(&updated.process_path).is_file()
            && let Some(aumid) = updated.aumid.as_deref()
            && let Some(root) = self.resolver.installed_path_for_aumid(aumid)
            && let Some(name) = Path::new(&updated.process_path).file_name()
        {
            updated.process_path = root.join(name).to_string_lossy().into_owned();
        }
        updated
    }
}
