use std::path::Path;
use windows::Management::Deployment::{PackageManager, PackageTypes};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagedApp {
    pub name: String,
    pub installed_path: String,
    pub aumid: Option<String>,
    pub icon_path: Option<String>,
}

#[derive(Default)]
pub struct PackagedApps {
    cached: Option<Vec<PackagedApp>>,
}

impl PackagedApps {
    pub fn installed(&mut self) -> &[PackagedApp] {
        if self.cached.is_none() {
            self.cached = Some(enumerate_packages());
        }
        self.cached.as_deref().unwrap_or(&[])
    }

    pub fn contains_path(&mut self, process_path: &str) -> bool {
        let process = Path::new(process_path);
        self.installed()
            .iter()
            .any(|package| process.starts_with(Path::new(&package.installed_path)))
    }

    pub fn searchable(&mut self) -> Vec<PackagedApp> {
        self.installed()
            .iter()
            .filter(|app| !app.name.is_empty() && app.aumid.is_some())
            .cloned()
            .collect()
    }
}

fn enumerate_packages() -> Vec<PackagedApp> {
    let Ok(manager) = PackageManager::new() else {
        return Vec::new();
    };
    let Ok(packages) = manager.FindPackagesWithPackageTypes(PackageTypes::Main) else {
        return Vec::new();
    };
    let apps = packages
        .into_iter()
        .filter_map(|package| {
            let name = package.DisplayName().ok()?.to_string();
            if name.is_empty() || name.starts_with("ms-resource:") {
                return None;
            }
            let path = package.InstalledPath().ok()?.to_string();
            let aumid = package
                .GetAppListEntries()
                .ok()?
                .into_iter()
                .next()
                .and_then(|entry| entry.AppInfo().ok())
                .and_then(|info| info.AppUserModelId().ok())
                .map(|value| value.to_string());
            let icon_path = package
                .Logo()
                .ok()
                .and_then(|uri| uri.Path().ok())
                .map(|value| value.to_string())
                .filter(|value| !value.is_empty());
            Some(PackagedApp {
                name,
                installed_path: path,
                aumid,
                icon_path,
            })
        })
        .collect::<Vec<_>>();
    crate::logging::debug(format!("Found {} installed packaged apps", apps.len()));
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_matching_accepts_package_children() {
        let mut packages = PackagedApps {
            cached: Some(vec![PackagedApp {
                name: "Demo".into(),
                installed_path: r"C:\Apps\Demo".into(),
                aumid: Some("Demo!App".into()),
                icon_path: None,
            }]),
        };
        assert!(packages.contains_path(r"C:\Apps\Demo\bin\demo.exe"));
        assert!(!packages.contains_path(r"C:\Apps\Other\demo.exe"));
    }
}
