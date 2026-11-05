use super::ApplicationWindow;
use crate::configuration::{ApplicationConfiguration, CycleMode};
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM},
    Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmGetWindowAttribute},
    System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    },
    UI::WindowsAndMessaging::{
        EnumWindows, GWL_EXSTYLE, GetForegroundWindow, GetShellWindow, GetWindowLongPtrW,
        GetWindowPlacement, GetWindowTextLengthW, GetWindowTextW, IsIconic, IsWindow,
        IsWindowVisible, WINDOWPLACEMENT,
    },
};
use windows::core::{BOOL, PWSTR};

pub struct WindowEnumerator {
    cached: Option<(Instant, Vec<ApplicationWindow>)>,
}

impl Default for WindowEnumerator {
    fn default() -> Self {
        Self { cached: None }
    }
}

impl WindowEnumerator {
    pub fn get_windows(&mut self) -> Vec<ApplicationWindow> {
        let _timing = crate::logging::measure_time("WindowDiscovery.GetWindows");
        let shell = unsafe { GetShellWindow() };
        let mut state = (Vec::new(), shell);
        unsafe {
            EnumWindows(
                Some(enumerate_window),
                LPARAM((&mut state as *mut _) as isize),
            )
            .ok();
        }
        state
            .0
            .into_iter()
            .filter_map(|hwnd| self.read_window(hwnd))
            .filter(ApplicationWindow::is_valid)
            .collect()
    }
    pub fn get_all_windows(&mut self) -> Vec<ApplicationWindow> {
        let _timing = crate::logging::measure_time("WindowDiscovery.GetAllWindows");
        let shell = unsafe { GetShellWindow() };
        let mut state = (Vec::new(), shell);
        unsafe {
            EnumWindows(
                Some(enumerate_window),
                LPARAM((&mut state as *mut _) as isize),
            )
            .ok();
        }
        state
            .0
            .into_iter()
            .filter_map(|hwnd| self.read_window(hwnd))
            .collect()
    }

    pub fn log_all_windows(&mut self) -> Result<PathBuf, String> {
        let windows = self.get_all_windows();
        let directory = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("OmniSpot")
            .join("logs");
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let path = directory.join("omnispot.log");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        writeln!(
            file,
            "--- OmniSpot window snapshot ({:?}) ---",
            std::time::SystemTime::now()
        )
        .map_err(|error| error.to_string())?;
        for window in windows {
            writeln!(
                file,
                "hwnd={} pid={} minimized={} cloaked={} elevated={} process={} path={} title={}",
                window.handle,
                window.process_id,
                window.is_minimized,
                window.is_cloaked,
                window.needs_elevation,
                window.process_name(),
                window.process_image_path,
                window.title.replace(['\r', '\n'], " ")
            )
            .map_err(|error| error.to_string())?;
        }
        Ok(path)
    }

    pub fn get_total_choices_count(&mut self) -> usize {
        self.get_cached_windows().len()
    }
    pub fn get_cached_windows(&mut self) -> Vec<ApplicationWindow> {
        if let Some((at, windows)) = &self.cached {
            if at.elapsed() < Duration::from_secs(10) {
                return windows.clone();
            }
        }
        let windows = self.get_windows();
        self.cached = Some((Instant::now(), windows.clone()));
        windows
    }
    pub fn get_current_window(&mut self) -> Option<ApplicationWindow> {
        let mut hwnd = unsafe { GetForegroundWindow() };
        while unsafe { GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWL_STYLE) }
            as u32
            & 0x40000000
            != 0
        {
            hwnd = unsafe { windows::Win32::UI::WindowsAndMessaging::GetParent(hwnd) }
                .ok()
                .filter(|parent| !parent.0.is_null())?;
            if hwnd.0.is_null() {
                return None;
            }
        }
        self.read_window(hwnd)
    }
    pub fn get_focused_app_windows(
        &mut self,
        all: &[ApplicationWindow],
        apps: &[ApplicationConfiguration],
    ) -> Vec<ApplicationWindow> {
        let current = match self.get_current_window() {
            Some(value) => value,
            None => return Vec::new(),
        };
        let app = match apps.iter().find(|app| {
            app.process_name()
                .eq_ignore_ascii_case(current.process_name())
                && app.cycle_mode == CycleMode::NextWindow
        }) {
            Some(value) => value,
            None => return Vec::new(),
        };
        let mut result = all
            .iter()
            .filter(|window| {
                window
                    .process_name()
                    .eq_ignore_ascii_case(app.process_name())
            })
            .cloned()
            .collect::<Vec<_>>();
        result.sort_by_key(|window| window.handle);
        result.truncate(10);
        result
    }
    fn read_window(&self, hwnd: HWND) -> Option<ApplicationWindow> {
        if hwnd.0.is_null() {
            return None;
        }
        let title = get_window_text(hwnd);
        if title.is_empty() {
            return None;
        }
        let mut process_id = 0u32;
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                hwnd,
                Some(&mut process_id),
            );
        }
        let path = process_path(process_id)?;
        let extended_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 };
        let style = unsafe {
            GetWindowLongPtrW(hwnd, windows::Win32::UI::WindowsAndMessaging::GWL_STYLE) as u32
        };
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe {
            let _ = GetWindowPlacement(hwnd, &mut placement);
        }
        let rect = placement.rcNormalPosition;
        let mut cloaked = 0i32;
        let _ = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(14),
                (&mut cloaked as *mut i32).cast(),
                std::mem::size_of::<i32>() as u32,
            )
        };
        Some(ApplicationWindow {
            handle: hwnd.0 as isize,
            title,
            process_id,
            process_image_path: path,
            is_minimized: unsafe { IsIconic(hwnd).as_bool() },
            is_cloaked: cloaked != 0,
            extended_style,
            style,
            position: (rect.left, rect.top),
            size: (rect.right - rect.left, rect.bottom - rect.top),
            needs_elevation:
                crate::window_discovery::process_inspector::ProcessInspector::needs_elevation(
                    process_id,
                ),
        })
    }
}

unsafe extern "system" fn enumerate_window(hwnd: HWND, data: LPARAM) -> BOOL {
    let state = &mut *(data.0 as *mut (Vec<HWND>, HWND));
    if hwnd != state.1 && IsWindowVisible(hwnd).as_bool() && IsWindow(Some(hwnd)).as_bool() {
        state.0.push(hwnd);
    }
    BOOL(1)
}

fn get_window_text(hwnd: HWND) -> String {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length == 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let count = GetWindowTextW(hwnd, &mut buffer);
        OsString::from_wide(&buffer[..count as usize])
            .to_string_lossy()
            .into_owned()
    }
}

fn process_path(process_id: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()?;
        let mut buffer = vec![0u16; 1024];
        let mut length = buffer.len() as u32;
        QueryFullProcessImageNameW(
            process,
            windows::Win32::System::Threading::PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
        .ok()?;
        Some(
            OsString::from_wide(&buffer[..length as usize])
                .to_string_lossy()
                .into_owned(),
        )
    }
}
