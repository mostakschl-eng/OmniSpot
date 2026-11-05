use std::error::Error;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize, IPersistFile,
};
use windows::Win32::UI::Shell::{FOLDERID_Startup, IShellLinkW, SHGetKnownFolderPath, ShellLink};
use windows::core::{Interface, PCWSTR};

pub(crate) struct AutoStart;

impl AutoStart {
    pub(crate) fn is_enabled() -> Result<bool, Box<dyn Error>> {
        Ok(link_path()?.exists())
    }

    pub(crate) fn remove_shortcut() -> Result<(), Box<dyn Error>> {
        let path = link_path()?;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    pub(crate) fn create_shortcut() -> Result<(), Box<dyn Error>> {
        let executable = std::env::current_exe()?;
        let shortcut = link_path_for(&executable)?;
        create_shortcut_at(&executable, &shortcut)
    }
}

fn create_shortcut_at(executable: &Path, shortcut: &Path) -> Result<(), Box<dyn Error>> {
    let working_directory = executable
        .parent()
        .ok_or_else(|| io::Error::other("executable has no parent directory"))?;
    let target = wide(executable);
    let directory = wide(working_directory);
    let save_path = wide(shortcut);

    // ShellLink is a COM object; only undo initialization performed by this call.
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if initialized.is_err() && initialized != windows::Win32::Foundation::RPC_E_CHANGED_MODE {
        initialized.ok()?;
    }
    let result = (|| -> windows::core::Result<()> {
        let link: IShellLinkW =
            unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)? };
        unsafe {
            link.SetPath(PCWSTR(target.as_ptr()))?;
            link.SetWorkingDirectory(PCWSTR(directory.as_ptr()))?;
            link.cast::<IPersistFile>()?
                .Save(PCWSTR(save_path.as_ptr()), true)?;
        }
        Ok(())
    })();
    if initialized.is_ok() {
        unsafe { CoUninitialize() };
    }
    result?;
    Ok(())
}

fn link_path() -> Result<PathBuf, Box<dyn Error>> {
    link_path_for(&std::env::current_exe()?)
}

fn link_path_for(executable: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let name = shortcut_name(executable)?;
    let startup = unsafe { SHGetKnownFolderPath(&FOLDERID_Startup, Default::default(), None)? };
    let folder = unsafe { startup.to_string() };
    unsafe { CoTaskMemFree(Some(startup.0.cast())) };
    Ok(PathBuf::from(folder?).join(name))
}

fn shortcut_name(executable: &Path) -> io::Result<std::ffi::OsString> {
    let mut name = executable
        .file_stem()
        .ok_or_else(|| io::Error::other("executable has no file name"))?
        .to_os_string();
    name.push(".lnk");
    Ok(name)
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::{create_shortcut_at, shortcut_name, wide};
    use std::path::Path;

    #[test]
    fn wide_path_is_null_terminated() {
        let path = wide(Path::new(r"C:\Apps\OmniSpot.exe"));
        assert_eq!(path.last(), Some(&0));
        assert_eq!(
            String::from_utf16(&path[..path.len() - 1]).unwrap(),
            r"C:\Apps\OmniSpot.exe"
        );
    }

    #[test]
    fn shortcut_name_preserves_dots_in_executable_name() {
        assert_eq!(
            shortcut_name(Path::new("OmniSpot.beta.exe")).unwrap(),
            "OmniSpot.beta.lnk"
        );
    }

    #[test]
    fn creates_shortcut_outside_startup_folder() {
        let executable = std::env::current_exe().unwrap();
        let shortcut =
            std::env::temp_dir().join(format!("omnispot-shortcut-test-{}.lnk", std::process::id()));
        create_shortcut_at(&executable, &shortcut).unwrap();
        assert!(shortcut.is_file());
        std::fs::remove_file(shortcut).unwrap();
    }
}
