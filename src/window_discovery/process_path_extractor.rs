use super::warn;
use windows::core::{HRESULT, PWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, HANDLE};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};

pub(super) struct OwnedHandle(pub HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub(super) fn get_process_image_path(process_id: u32) -> Option<String> {
    let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }
    {
        Ok(handle) => OwnedHandle(handle),
        Err(_) => {
            warn(format!("Failed to open process {process_id}"));
            return None;
        }
    };
    let mut buffer = vec![0u16; 261];
    loop {
        let mut length = buffer.len() as u32;
        unsafe {
            match QueryFullProcessImageNameW(
                handle.0,
                PROCESS_NAME_FORMAT(0),
                PWSTR(buffer.as_mut_ptr()),
                &mut length,
            ) {
                Ok(()) => return Some(String::from_utf16_lossy(&buffer[..length as usize])),
                Err(error) if error.code() == HRESULT::from_win32(ERROR_INSUFFICIENT_BUFFER.0) => {}
                Err(_) => {
                    warn(format!(
                        "Failed to get process image name for process {process_id}"
                    ));
                    return None;
                }
            }
        }
        buffer.resize(buffer.len() * 2, 0);
    }
}
