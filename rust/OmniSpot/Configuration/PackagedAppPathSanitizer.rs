use super::model::{ApplicationConfiguration, ApplicationType, Configuration};
use std::path::{Path, PathBuf};

pub trait PackagedAppResolver {
    fn package_family_for_path(&self, path: &Path) -> Option<String>;
    fn aumid_for_family(&self, family: &str) -> Option<String>;
    fn installed_path_for_aumid(&self, aumid: &str) -> Option<PathBuf>;
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
        if app.application_type == ApplicationType::Win32 {
            if let Some(family) = self
                .resolver
                .package_family_for_path(Path::new(&app.process_path))
            {
                if let Some(aumid) = self.resolver.aumid_for_family(&family) {
                    updated.application_type = ApplicationType::Packaged;
                    updated.aumid = Some(aumid);
                }
            }
        }
        if updated.application_type == ApplicationType::Packaged
            && !Path::new(&updated.process_path).is_file()
        {
            if let Some(aumid) = updated.aumid.as_deref() {
                if let Some(root) = self.resolver.installed_path_for_aumid(aumid) {
                    if let Some(name) = Path::new(&updated.process_path).file_name() {
                        updated.process_path = root.join(name).to_string_lossy().into_owned();
                    }
                }
            }
        }
        updated
    }
}
