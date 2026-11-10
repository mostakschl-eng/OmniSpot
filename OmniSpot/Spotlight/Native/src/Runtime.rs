use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Com::{
    CoGetApartmentType, CoInitializeEx, CoUninitialize, APTTYPE, APTTYPEQUALIFIER, APTTYPE_MAINSTA,
    APTTYPE_STA, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Threading::{
    CreateProcessW, CREATE_UNICODE_ENVIRONMENT, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::Win32::UI::Shell::{
    ShellExecuteExW, SEE_MASK_FLAG_DDEWAIT, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS,
    SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

// Native equivalents of .NET process-launch APIs; no application selection logic.
fn command_line(path: &str, arguments: Option<&str>) -> String {
    let path = path.trim();
    let mut command = if path.starts_with('"') && path.ends_with('"') {
        path.to_string()
    } else {
        format!("\"{path}\"")
    };
    if let Some(arguments) = arguments.filter(|a| !a.is_empty()) {
        command.push(' ');
        command.push_str(arguments);
    }
    command
}

pub(crate) fn start_process(path: &str, arguments: Option<&str>) -> Result<(), RuntimeError> {
    if path.is_empty() {
        return Err(RuntimeError::invalid("No process file name was specified"));
    }
    let mut command: Vec<_> = command_line(path, arguments)
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR::null(),
            PWSTR(command.as_mut_ptr()),
            None,
            None,
            true,
            CREATE_UNICODE_ENVIRONMENT,
            None,
            PCWSTR::null(),
            &startup,
            &mut process,
        )
        .map_err(RuntimeError::launch)?;
        let _ = CloseHandle(process.hThread);
        let _ = CloseHandle(process.hProcess);
    }
    Ok(())
}

pub(crate) fn shell_open(target: &str) -> Result<(), RuntimeError> {
    if target.is_empty() {
        return Err(RuntimeError::invalid("No process file name was specified"));
    }
    let mut apartment = APTTYPE::default();
    let mut qualifier = APTTYPEQUALIFIER::default();
    if unsafe { CoGetApartmentType(&mut apartment, &mut qualifier) }.is_ok()
        && matches!(apartment, APTTYPE_STA | APTTYPE_MAINSTA)
    {
        return shell_open_on_sta(target);
    }
    let target = target.to_string();
    std::thread::Builder::new()
        .spawn(move || {
            unsafe {
                CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                    .ok()
                    .map_err(RuntimeError::launch)?;
            }
            let result = shell_open_on_sta(&target);
            unsafe {
                CoUninitialize();
            }
            result
        })
        .map_err(|error| RuntimeError::invalid(&error.to_string()))?
        .join()
        .map_err(|_| RuntimeError::invalid("The shell-launch thread failed"))?
}

fn shell_open_on_sta(target: &str) -> Result<(), RuntimeError> {
    let target: Vec<_> = target.encode_utf16().chain(Some(0)).collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_DDEWAIT | SEE_MASK_FLAG_NO_UI,
        lpFile: PCWSTR(target.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info).map_err(RuntimeError::launch)?;
        if !info.hProcess.is_invalid() {
            let _ = CloseHandle(info.hProcess);
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct RuntimeError {
    pub message: String,
    pub win32_error: Option<i32>,
}

impl RuntimeError {
    pub(crate) fn invalid(message: &str) -> Self {
        Self {
            message: message.into(),
            win32_error: None,
        }
    }
    fn launch(error: windows::core::Error) -> Self {
        Self {
            message: error.to_string(),
            win32_error: Some(error.code().0 & 0xffff),
        }
    }
}

impl From<windows::core::Error> for RuntimeError {
    fn from(error: windows::core::Error) -> Self {
        Self::launch(error)
    }
}

pub(crate) fn file_name(path: &str) -> &str {
    let prefix = match std::path::Path::new(path).components().next() {
        Some(std::path::Component::Prefix(prefix)) => prefix.as_os_str().len(),
        _ => 0,
    };
    let start = path.rfind(['\\', '/']).map_or(0, |i| i + 1).max(prefix);
    &path[start..]
}

#[cfg(test)]
#[path = "tests/RuntimeTests.rs"]
mod tests;
