use std::{ffi::OsStr, os::windows::ffi::OsStrExt, path::Path};
use windows::Win32::{
    Foundation::{ERROR_ACCESS_DENIED, GetLastError},
    Storage::FileSystem::GetBinaryTypeW,
    System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION},
    UI::Shell::{ASSOCF_OPEN_BYEXENAME, ASSOCSTR_FRIENDLYAPPNAME, AssocQueryStringW},
};
use windows::core::PCWSTR;

/// Windows process checks used when a target application is protected or
/// cannot be queried through the normal limited-information handle.
pub(crate) struct ProcessInspector;

impl ProcessInspector {
    pub(crate) fn needs_elevation(process_id: u32) -> bool {
        match unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, false, process_id) } {
            Ok(handle) => {
                unsafe {
                    let _ = windows::Win32::Foundation::CloseHandle(handle);
                }
                false
            }
            Err(_) => unsafe { GetLastError() == ERROR_ACCESS_DENIED },
        }
    }

    pub(crate) fn is_windows_executable(path: &str) -> bool {
        let wide = wide(path);
        let mut binary_type = 0u32;
        let valid = unsafe { GetBinaryTypeW(PCWSTR(wide.as_ptr()), &mut binary_type).is_ok() };
        valid && matches!(binary_type, 0 | 6)
    }

    pub(crate) fn display_name(path: &str) -> String {
        if let Some(name) = associated_display_name(path) {
            return name;
        }
        Path::new(path)
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(path)
            .to_string()
    }
}

fn associated_display_name(path: &str) -> Option<String> {
    let path_wide = wide(path);
    let mut buffer = vec![0u16; 512];
    let mut length = buffer.len() as u32;
    let result = unsafe {
        AssocQueryStringW(
            ASSOCF_OPEN_BYEXENAME,
            ASSOCSTR_FRIENDLYAPPNAME,
            PCWSTR(path_wide.as_ptr()),
            PCWSTR::null(),
            Some(windows::core::PWSTR(buffer.as_mut_ptr())),
            &mut length,
        )
    };
    if result.is_ok() && length > 1 {
        return String::from_utf16(&buffer[..length.saturating_sub(1) as usize]).ok();
    }
    None
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::ProcessInspector;

    #[test]
    fn display_name_uses_executable_stem() {
        assert_eq!(
            ProcessInspector::display_name(r"C:\Apps\Editor.exe"),
            "Editor"
        );
    }
}
