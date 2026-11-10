use super::process_path_extractor::OwnedHandle;
use super::{equal_ignore_case, file_stem, warn, wide};
use std::time::{Duration, Instant};
use windows::core::{HRESULT, PCWSTR, PWSTR};
use windows::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows::Win32::Storage::FileSystem::{
    GetBinaryTypeW, GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION};
use windows::Win32::UI::Shell::{
    AssocQueryStringW, ASSOCF_OPEN_BYEXENAME, ASSOCSTR_FRIENDLYAPPNAME,
};

pub(super) fn needs_elevation(process_id: u32) -> bool {
    match unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, false, process_id) } {
        Ok(handle) => {
            drop(OwnedHandle(handle));
            false
        }
        Err(error) => {
            if error.code() == HRESULT::from_win32(ERROR_ACCESS_DENIED.0) {
                return true;
            }
            warn(format!(
                "Failed to open process (PROCESS_QUERY_INFORMATION) - error code: {}",
                error.code().0 & 0xffff
            ));
            false
        }
    }
}

pub(super) fn wait_for_potential_elevation(path: &str, timeout_ms: i32) -> Result<bool, String> {
    let process_name = file_stem(path);
    let start = Instant::now();
    while start.elapsed().as_millis() < timeout_ms.max(0) as u128 {
        let snapshot = OwnedHandle(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
                .map_err(|e| e.to_string())?,
        );
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut has_entry = unsafe { Process32FirstW(snapshot.0, &mut entry) }.is_ok();
        while has_entry {
            let length = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..length]);
            if equal_ignore_case(file_stem(&name), process_name)
                && needs_elevation(entry.th32ProcessID)
            {
                return Ok(true);
            }
            has_entry = unsafe { Process32NextW(snapshot.0, &mut entry) }.is_ok();
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(false)
}

pub(super) fn is_windows_executable(path: &str) -> bool {
    let path = wide(path);
    let mut kind = 0;
    unsafe { GetBinaryTypeW(PCWSTR(path.as_ptr()), &mut kind).is_ok() && matches!(kind, 0 | 6) }
}

pub(super) fn get_process_display_name(path: &str) -> String {
    let wide_path = wide(path);
    let mut length = 260;
    let mut buffer = [0u16; 260];
    let result = unsafe {
        AssocQueryStringW(
            ASSOCF_OPEN_BYEXENAME,
            ASSOCSTR_FRIENDLYAPPNAME,
            PCWSTR(wide_path.as_ptr()),
            PCWSTR::null(),
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    let mut display_name = if result.is_ok() && length > 0 && length <= buffer.len() as u32 {
        String::from_utf16_lossy(&buffer[..length as usize - 1])
    } else {
        warn(format!(
            "Failed to get display name for process {path} - error code: {result}"
        ));
        file_stem(path).to_string()
    };
    if std::path::Path::new(path).is_file() {
        if let Some(description) = version_string(path, "FileDescription") {
            if !description.is_empty()
                && description.encode_utf16().count() < display_name.encode_utf16().count()
            {
                display_name = description;
            }
        }
    }
    display_name
}

pub(crate) fn get_product_name(path: &str) -> windows::core::Result<Option<String>> {
    if !std::path::Path::new(path).is_file() {
        return Err(windows::core::Error::new(
            HRESULT::from_win32(2),
            format!("File not found: {path}"),
        ));
    }
    Ok(version_string(path, "ProductName"))
}

fn version_string(path: &str, field: &str) -> Option<String> {
    let path = wide(path);
    unsafe {
        let length = GetFileVersionInfoSizeW(PCWSTR(path.as_ptr()), None);
        if length == 0 {
            return None;
        }
        let mut data = vec![0u8; length as usize];
        GetFileVersionInfoW(PCWSTR(path.as_ptr()), 0, length, data.as_mut_ptr().cast()).ok()?;
        let translation = wide("\\VarFileInfo\\Translation");
        let mut pointer = std::ptr::null_mut();
        let mut size = 0;
        let mut languages = Vec::new();
        if VerQueryValueW(
            data.as_ptr().cast(),
            PCWSTR(translation.as_ptr()),
            &mut pointer,
            &mut size,
        )
        .as_bool()
            && size >= 4
        {
            let words = std::slice::from_raw_parts(pointer.cast::<u16>(), 2);
            languages.push((words[0], words[1]));
        }
        languages.extend([(0x0409, 0x04b0), (0x0409, 0x04e4), (0x0409, 0)]);
        for (language, code_page) in languages {
            let key = wide(&format!(
                "\\StringFileInfo\\{language:04x}{code_page:04x}\\{field}"
            ));
            if VerQueryValueW(
                data.as_ptr().cast(),
                PCWSTR(key.as_ptr()),
                &mut pointer,
                &mut size,
            )
            .as_bool()
                && size > 0
            {
                let value = std::slice::from_raw_parts(pointer.cast::<u16>(), size as usize);
                return Some(String::from_utf16_lossy(
                    value.strip_suffix(&[0]).unwrap_or(value),
                ));
            }
        }
    }
    Some(String::new())
}
