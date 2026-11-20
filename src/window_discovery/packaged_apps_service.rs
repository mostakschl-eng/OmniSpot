use super::process_path_extractor::OwnedHandle;
use super::{equal_ignore_case, starts_with_ignore_case, warn, wide};
use serde::Serialize;
use std::collections::HashMap;
use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::ApplicationModel::Package;
use windows::Management::Deployment::{PackageManager, PackageTypes};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Shell::PathCreateFromUrlW;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackagedAppInfo {
    pub aumid: String,
    pub icon_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchableApp {
    pub name: String,
    pub path: String,
    pub aumid: String,
    pub icon_path: String,
}

#[derive(Default)]
pub(crate) struct PackagedAppsService {
    cache: HashMap<String, PackagedAppInfo>,
}

fn packages() -> windows::core::Result<Vec<Package>> {
    let manager = PackageManager::new()?;
    Ok(manager
        .FindPackagesByUserSecurityIdWithPackageTypes(&HSTRING::new(), PackageTypes::Main)?
        .into_iter()
        .collect())
}

impl PackagedAppsService {
    pub fn get_searchable_apps(&mut self) -> windows::core::Result<Vec<SearchableApp>> {
        let mut result = Vec::new();
        for package in packages()? {
            let app = self.get_packaged_app_info(&package, None, None)?;
            let name = package.DisplayName()?.to_string();
            let path = package.InstalledPath()?.to_string();
            let id = package.Id()?;
            let family_name = id.FamilyName()?.to_string();
            let package_name = id.Name()?.to_string();
            if !name.trim().is_empty()
                && !starts_with_ignore_case(&name, "ms-resource:")
                && !equal_ignore_case(&name, &family_name)
                && !equal_ignore_case(&name, &package_name)
            {
                if let Some(app) = app {
                    result.push(SearchableApp {
                        name,
                        path,
                        aumid: app.aumid,
                        icon_path: app.icon_path,
                    });
                }
            }
        }
        Ok(result)
    }

    pub fn get_installed_packages(&self) -> windows::core::Result<Vec<String>> {
        packages()?
            .iter()
            .map(|p| p.InstalledPath().map(|s| s.to_string()))
            .collect()
    }

    pub fn get_display_name(&self, path: &str) -> windows::core::Result<Option<String>> {
        for package in packages()? {
            if starts_with_ignore_case(path, &package.InstalledPath()?.to_string()) {
                return Ok(Some(package.DisplayName()?.to_string()));
            }
        }
        Ok(None)
    }

    pub fn get_by_installed_path(
        &mut self,
        path: &str,
        process_id: Option<u32>,
    ) -> windows::core::Result<Option<PackagedAppInfo>> {
        for package in packages()? {
            if starts_with_ignore_case(path, &package.InstalledPath()?.to_string()) {
                return self.get_packaged_app_info(&package, process_id, None);
            }
        }
        Ok(None)
    }

    fn get_package_by_family(family: &str) -> windows::core::Result<Option<Package>> {
        Ok(PackageManager::new()?
            .FindPackagesByUserSecurityIdPackageFamilyName(&HSTRING::new(), &HSTRING::from(family))?
            .into_iter()
            .next())
    }

    fn get_package_by_aumid(aumid: Option<&str>) -> windows::core::Result<Option<Package>> {
        let Some(aumid) = aumid.filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        Self::get_package_by_family(aumid.split('!').next().unwrap_or_default())
    }

    pub fn get_installed_path_by_aumid(
        &self,
        aumid: Option<&str>,
    ) -> windows::core::Result<Option<String>> {
        Self::get_package_by_aumid(aumid)?
            .map(|p| p.InstalledPath().map(|s| s.to_string()))
            .transpose()
    }

    pub fn get_by_aumid(
        &mut self,
        aumid: Option<&str>,
    ) -> windows::core::Result<Option<PackagedAppInfo>> {
        match Self::get_package_by_aumid(aumid)? {
            Some(package) => self.get_packaged_app_info(&package, None, aumid),
            None => Ok(None),
        }
    }

    pub fn get_by_package_family_name(
        &mut self,
        family: &str,
    ) -> windows::core::Result<Option<PackagedAppInfo>> {
        match Self::get_package_by_family(family)? {
            Some(package) => self.get_packaged_app_info(&package, None, None),
            None => Ok(None),
        }
    }

    fn get_packaged_app_info(
        &mut self,
        package: &Package,
        process_id: Option<u32>,
        fallback: Option<&str>,
    ) -> windows::core::Result<Option<PackagedAppInfo>> {
        let path = package.InstalledPath()?.to_string();
        if let Some(cached) = self.cache.get(&path) {
            return Ok(Some(cached.clone()));
        }
        let entries = package.GetAppListEntries()?;
        let aumid = if entries.Size()? > 0 {
            Some(entries.GetAt(0)?.AppUserModelId()?.to_string())
        } else {
            fallback
                .map(str::to_string)
                .or_else(|| process_id.and_then(get_aumid_by_process_id))
        };
        let Some(aumid) = aumid else {
            return Ok(None);
        };
        let logo = package.Logo()?;
        let icon_path = if equal_ignore_case(&logo.SchemeName()?.to_string(), "file") {
            let uri = wide(&logo.AbsoluteUri()?.to_string());
            let mut buffer = vec![0u16; uri.len() + 1];
            let mut length = buffer.len() as u32;
            unsafe {
                PathCreateFromUrlW(
                    PCWSTR(uri.as_ptr()),
                    PWSTR(buffer.as_mut_ptr()),
                    &mut length,
                    0,
                )?;
            }
            String::from_utf16_lossy(&buffer[..length as usize])
        } else {
            String::new()
        };
        let info = PackagedAppInfo { aumid, icon_path };
        self.cache.insert(path, info.clone());
        Ok(Some(info))
    }
}

fn get_aumid_by_process_id(process_id: u32) -> Option<String> {
    let process = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }
    {
        Ok(handle) => OwnedHandle(handle),
        Err(error) => {
            warn(format!(
                "Failed to open process {process_id} to query AUMID: {error}"
            ));
            return None;
        }
    };
    let mut length = 0;
    let mut result = unsafe { GetApplicationUserModelId(process.0, &mut length, PWSTR::null()) };
    if result == ERROR_INSUFFICIENT_BUFFER {
        let mut buffer = vec![0u16; length as usize];
        result = unsafe {
            GetApplicationUserModelId(process.0, &mut length, PWSTR(buffer.as_mut_ptr()))
        };
        if result == ERROR_SUCCESS && length > 0 {
            return Some(String::from_utf16_lossy(&buffer[..length as usize - 1]));
        }
    }
    warn(format!(
        "Failed to get AUMID for process {process_id}: {}",
        result.0
    ));
    None
}
