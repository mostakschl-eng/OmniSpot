use std::io::Read;
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONULL,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::{
    SHAppBarMessage, ABM_GETSTATE, ABM_SETSTATE, ABS_AUTOHIDE, APPBARDATA,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetWindowThreadProcessId, IsWindowVisible, SetWindowPos, SWP_HIDEWINDOW,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
};

pub const TASKBAR_TEST_SECONDS: u64 = 20;

pub fn test_hide_windows_taskbar() -> Result<(), String> {
    let _lease = unsafe {
        let handle = CreateMutexW(None, false, w!("Local\\OmniSpotTaskbarPreview"))
            .map_err(|error| error.to_string())?;
        let already_open = GetLastError() == ERROR_ALREADY_EXISTS;
        let lease = OwnedHandle::from_raw_handle(handle.0);
        if already_open {
            return Err("A taskbar test is already running.".into());
        }
        lease
    };

    let mut hidden = HiddenTaskbars::default();
    hidden.hide()?;
    let (stop, stopped) = mpsc::channel();
    // The parent owns the writing end. Closing the preview or losing its process sends EOF.
    std::thread::spawn(move || {
        let _ = std::io::stdin().read(&mut [0]);
        let _ = stop.send(());
    });
    let _ = stopped.recv_timeout(Duration::from_secs(TASKBAR_TEST_SECONDS));
    hidden.restore()
}

#[derive(Default)]
struct HiddenTaskbars {
    windows: Vec<(HWND, u32)>,
    previous_appbar_state: Option<usize>,
}

impl HiddenTaskbars {
    fn hide(&mut self) -> Result<(), String> {
        let primary = primary_taskbar()?;
        // shortcut: Explorer's window class names are undocumented, replace this if Windows exposes a supported API.
        for class in [w!("Shell_TrayWnd"), w!("Shell_SecondaryTrayWnd")] {
            let mut previous = HWND::default();
            while let Ok(window) =
                unsafe { FindWindowExW(HWND::default(), previous, class, PCWSTR::null()) }
            {
                previous = window;
                let owner = owner_process(window);
                if owner != 0 && unsafe { IsWindowVisible(window).as_bool() } {
                    self.windows.push((window, owner));
                }
            }
        }
        if self.windows.is_empty() {
            return Err("No visible Windows taskbar was found.".into());
        }
        let previous_state = appbar_state();
        let auto_hide = ABS_AUTOHIDE as usize;
        if previous_state & auto_hide == 0 {
            let monitor = unsafe { MonitorFromWindow(primary, MONITOR_DEFAULTTONULL) };
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let original_work_area = unsafe { GetMonitorInfoW(monitor, &mut info) }
                .as_bool()
                .then_some(info.rcWork);
            self.previous_appbar_state = Some(previous_state);
            set_appbar_state(primary, previous_state | auto_hide);
            let deadline = Instant::now() + Duration::from_millis(1500);
            loop {
                let updated = unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool();
                let work_area_ready = original_work_area.is_none_or(|original| {
                    updated && (original == info.rcMonitor || info.rcWork != original)
                });
                if appbar_state() & auto_hide != 0 && work_area_ready {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("Windows did not release the taskbar's work area.".into());
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        for &(window, _) in &self.windows {
            set_visible(window, false)?;
        }
        Ok(())
    }

    fn restore(&mut self) -> Result<(), String> {
        self.windows.retain(|&(window, owner)| {
            owner_process(window) == owner && set_visible(window, true).is_err()
        });
        if let Some(previous) = self.previous_appbar_state {
            set_appbar_state(primary_taskbar()?, previous);
            if appbar_state() != previous {
                return Err("Windows did not restore the original taskbar settings.".into());
            }
            self.previous_appbar_state = None;
        }
        if !self.windows.is_empty() {
            return Err("Windows did not restore one or more taskbars.".into());
        }
        Ok(())
    }
}

impl Drop for HiddenTaskbars {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

fn owner_process(window: HWND) -> u32 {
    let mut process = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
    process
}

fn primary_taskbar() -> Result<HWND, String> {
    unsafe {
        FindWindowExW(
            HWND::default(),
            HWND::default(),
            w!("Shell_TrayWnd"),
            PCWSTR::null(),
        )
    }
    .map_err(|_| "The primary Windows taskbar was not found.".into())
}

fn appbar_state() -> usize {
    let mut data = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        ..Default::default()
    };
    unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) }
}

fn set_appbar_state(window: HWND, state: usize) {
    let mut data = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: window,
        lParam: LPARAM(state as isize),
        ..Default::default()
    };
    unsafe { SHAppBarMessage(ABM_SETSTATE, &mut data) };
}

fn set_visible(window: HWND, visible: bool) -> Result<(), String> {
    let visibility = if visible {
        SWP_SHOWWINDOW
    } else {
        SWP_HIDEWINDOW
    };
    let flags = visibility | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER;
    unsafe { SetWindowPos(window, HWND::default(), 0, 0, 0, 0, flags) }
        .map_err(|error| error.to_string())?;
    if unsafe { IsWindowVisible(window).as_bool() } == visible {
        Ok(())
    } else {
        Err("Windows did not apply the taskbar visibility change.".into())
    }
}
