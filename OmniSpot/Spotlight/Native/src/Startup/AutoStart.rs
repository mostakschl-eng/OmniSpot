use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

pub(crate) struct AutoStart {
    app_name: String,
    startup_dir_override: Option<PathBuf>,
}

impl AutoStart {
    pub(crate) fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            startup_dir_override: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_startup_dir(app_name: impl Into<String>, dir: PathBuf) -> Self {
        Self {
            app_name: app_name.into(),
            startup_dir_override: Some(dir),
        }
    }

    pub(crate) fn get_link_path(&self) -> Option<PathBuf> {
        if let Some(ref dir) = self.startup_dir_override {
            return Some(dir.join(format!("{}.lnk", self.app_name)));
        }

        #[cfg(windows)]
        {
            let app_data = std::env::var("APPDATA").ok()?;
            let startup = Path::new(&app_data)
                .join(r"Microsoft\Windows\Start Menu\Programs\Startup")
                .join(format!("{}.lnk", self.app_name));
            Some(startup)
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.get_link_path()
            .map(|p| p.is_file())
            .unwrap_or(false)
    }

    pub(crate) fn remove_shortcut(&self) -> bool {
        if let Some(link_path) = self.get_link_path() {
            if link_path.is_file() {
                std::fs::remove_file(link_path).is_ok()
            } else {
                true
            }
        } else {
            false
        }
    }

    pub(crate) fn create_shortcut(&self, target_exe: &Path) -> bool {
        let Some(link_path) = self.get_link_path() else {
            return false;
        };

        #[cfg(windows)]
        {
            use windows::core::{Interface, PCWSTR};
            use windows::Win32::System::Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
                COINIT_APARTMENTTHREADED, IPersistFile,
            };
            use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

                let shell_link: Result<IShellLinkW, _> =
                    CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER);

                let res = if let Ok(link) = shell_link {
                    let target_wide: Vec<u16> = target_exe
                        .as_os_str()
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();
                    let _ = link.SetPath(PCWSTR(target_wide.as_ptr()));

                    if let Some(parent) = target_exe.parent() {
                        let workdir_wide: Vec<u16> = parent
                            .as_os_str()
                            .encode_wide()
                            .chain(std::iter::once(0))
                            .collect();
                        let _ = link.SetWorkingDirectory(PCWSTR(workdir_wide.as_ptr()));
                    }

                    if let Ok(persist) = link.cast::<IPersistFile>() {
                        let link_wide: Vec<u16> = link_path
                            .as_os_str()
                            .encode_wide()
                            .chain(std::iter::once(0))
                            .collect();
                        persist.Save(PCWSTR(link_wide.as_ptr()), true).is_ok()
                    } else {
                        false
                    }
                } else {
                    false
                };

                CoUninitialize();
                res
            }
        }
        #[cfg(not(windows))]
        {
            let _ = target_exe;
            let _ = link_path;
            false
        }
    }
}

#[cfg(test)]
#[path = "../tests/Startup/AutoStartTests.rs"]
mod tests;
