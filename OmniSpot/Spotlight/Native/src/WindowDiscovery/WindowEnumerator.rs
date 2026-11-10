use super::application_window::ApplicationWindow;
use super::i_window_enumerator::IWindowEnumerator;
use super::process_inspector::needs_elevation;
use super::process_path_extractor::get_process_image_path;
use super::{ends_with_ignore_case, file_name, warn};
use crate::configuration::configuration::{ApplicationConfiguration, CycleMode};
use serde::Serialize;
use std::time::{Duration, SystemTime};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Default)]
pub(crate) struct WindowEnumerator {
    cache: Option<(SystemTime, Vec<ApplicationWindow>)>,
}

impl WindowEnumerator {
    pub fn get_cached_windows(&mut self) -> Vec<ApplicationWindow> {
        if let Some((expires, windows)) = &self.cache {
            if SystemTime::now() < *expires {
                return windows.clone();
            }
        }
        let windows = get_windows(true);
        self.cache = Some((SystemTime::now() + Duration::from_secs(10), windows.clone()));
        windows
    }
}

pub(crate) fn get_windows(valid_only: bool) -> Vec<ApplicationWindow> {
    let mut handles = Vec::<HWND>::new();
    unsafe {
        let _ = EnumWindows(
            Some(enumerate),
            LPARAM((&mut handles as *mut Vec<HWND>) as isize),
        );
    }
    handles
        .into_iter()
        .filter_map(get_application_window)
        .filter(|window| !valid_only || window.is_valid_window())
        .collect()
}

unsafe extern "system" fn enumerate(hwnd: HWND, parameter: LPARAM) -> BOOL {
    if hwnd != GetShellWindow() && IsWindowVisible(hwnd).as_bool() && IsWindow(hwnd).as_bool() {
        let handles = &mut *(parameter.0 as *mut Vec<HWND>);
        if !handles.contains(&hwnd) {
            handles.push(hwnd);
        }
    }
    BOOL(1)
}

pub(crate) fn get_current_window() -> Option<ApplicationWindow> {
    unsafe {
        let mut hwnd = GetForegroundWindow();
        while GetWindowLongW(hwnd, GWL_STYLE) as u32 & WS_CHILD.0 != 0 {
            hwnd = GetParent(hwnd).unwrap_or_default();
        }
        get_application_window(hwnd)
    }
}

fn get_application_window(hwnd: HWND) -> Option<ApplicationWindow> {
    unsafe {
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        let _ = GetWindowPlacement(hwnd, &mut placement);
        let rect = placement.rcNormalPosition;
        let width = rect.right.wrapping_sub(rect.left);
        let height = rect.bottom.wrapping_sub(rect.top);
        let length = GetWindowTextLengthW(hwnd).max(0) as usize;
        let title = if length == 0 {
            String::new()
        } else {
            let mut buffer = vec![0u16; if length <= 256 { 256 } else { length + 1 }];
            let copied = GetWindowTextW(hwnd, &mut buffer).max(0) as usize;
            String::from_utf16_lossy(&buffer[..copied])
        };
        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        if width == 0 && height == 0 {
            return None;
        }
        let process_image_path = get_process_image_path(process_id)?;
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let style_ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let mut cloaked = 0i32;
        if let Err(error) =
            DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, (&mut cloaked as *mut i32).cast(), 4)
        {
            warn(format!(
                "Failed to get cloaked attribute for window {} ({}), HRESULT: {}",
                hwnd.0 as isize,
                file_name(&process_image_path),
                error.code()
            ));
        }
        Some(ApplicationWindow {
            handle: hwnd.0 as isize as i64,
            title: Some(title),
            process_id,
            process_image_path,
            state: placement.showCmd,
            x: rect.left,
            y: rect.top,
            width,
            height,
            style,
            style_ex,
            is_cloaked: cloaked != 0,
            needs_elevation: needs_elevation(process_id),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FocusedAppWindows {
    pub focused_window: Option<ApplicationWindow>,
    pub all_windows: Vec<ApplicationWindow>,
}

pub(crate) fn get_focused_app_windows(
    current: Option<ApplicationWindow>,
    all: Vec<ApplicationWindow>,
    applications: &[ApplicationConfiguration],
) -> FocusedAppWindows {
    let Some(current) = current else {
        return FocusedAppWindows {
            focused_window: None,
            all_windows: vec![],
        };
    };
    let focused_app = applications.iter().find(|app| {
        super::process_name_matcher::invariant_equal(
            app.process_name(),
            file_name(&current.process_image_path),
        ) && app.cycle_mode == CycleMode::NextWindow
    });
    let Some(app) = focused_app else {
        return FocusedAppWindows {
            focused_window: None,
            all_windows: vec![],
        };
    };
    let mut windows: Vec<_> = all
        .into_iter()
        .filter(|w| ends_with_ignore_case(&w.process_image_path, app.process_name()))
        .collect();
    windows.sort_by_key(|w| w.handle);
    windows.truncate(10);
    FocusedAppWindows {
        focused_window: Some(current),
        all_windows: windows,
    }
}

impl IWindowEnumerator for WindowEnumerator {
    fn get_windows(&self) -> Vec<ApplicationWindow> {
        get_windows(true)
    }
    fn get_cached_windows(&mut self) -> Vec<ApplicationWindow> {
        WindowEnumerator::get_cached_windows(self)
    }
    fn get_current_window(&self) -> Option<ApplicationWindow> {
        get_current_window()
    }
    fn get_focused_app_windows(
        &self,
        all: &[ApplicationWindow],
        applications: &[ApplicationConfiguration],
    ) -> FocusedAppWindows {
        get_focused_app_windows(get_current_window(), all.to_vec(), applications)
    }
    fn get_all_windows(&self) -> Vec<ApplicationWindow> {
        get_windows(false)
    }
}

#[cfg(test)]
#[path = "../tests/WindowDiscovery/WindowEnumeratorTests.rs"]
mod tests;
