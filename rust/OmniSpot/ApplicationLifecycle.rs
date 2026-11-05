use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::PCWSTR;

pub(crate) struct SingleInstance {
    handle: HANDLE,
}

impl SingleInstance {
    pub(crate) fn acquire() -> Result<Self, String> {
        let name: Vec<u16> = OsStr::new("OmniSpotMutex")
            .encode_wide()
            .chain(Some(0))
            .collect();
        let handle = unsafe { CreateMutexW(None, true, PCWSTR(name.as_ptr())) }
            .map_err(|error| format!("Could not create the single-instance mutex: {error}"))?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
            return Err("Another instance of OmniSpot is already running".to_string());
        }
        Ok(Self { handle })
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}
