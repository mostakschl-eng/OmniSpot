use crate::{
    configuration::{ApplicationConfiguration, ApplicationType, CycleMode},
    window_discovery::{ApplicationWindow, WindowEnumerator},
};
use std::{path::Path, process::Command};
use windows::Win32::{
    Foundation::HWND,
    System::Threading::AttachThreadInput,
    UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, IsIconic, SW_MINIMIZE, SW_RESTORE,
        SetForegroundWindow, ShowWindow, SwitchToThisWindow,
    },
};

#[derive(Debug, Clone)]
pub struct AppSwitchResult {
    pub process_id: Option<u32>,
    pub process_path: String,
    pub handle: Option<isize>,
    pub was_started: bool,
}

pub struct Switcher {
    pub windows: WindowEnumerator,
    next_windows: Vec<isize>,
}
impl Default for Switcher {
    fn default() -> Self {
        Self {
            windows: WindowEnumerator::default(),
            next_windows: Vec::new(),
        }
    }
}

impl Switcher {
    pub fn activate_handle(handle: isize) {
        let mut enumerator = WindowEnumerator::default();
        if let Some(window) = enumerator
            .get_all_windows()
            .into_iter()
            .find(|window| window.handle == handle)
        {
            activate(&window);
        }
    }

    pub fn hide_handle(handle: isize) {
        hide(handle);
    }

    pub fn execute(&mut self, group: &[ApplicationConfiguration]) -> Option<AppSwitchResult> {
        if group.is_empty() {
            return None;
        }
        let index = self
            .windows
            .get_current_window()
            .and_then(|current| {
                group.iter().position(|app| {
                    app.process_name()
                        .eq_ignore_ascii_case(current.process_name())
                })
            })
            .unwrap_or(usize::MAX);
        self.execute_one(
            &group[if group.len() == 1 {
                0
            } else {
                (index.wrapping_add(1)) % group.len()
            }],
        )
    }
    fn execute_one(&mut self, app: &ApplicationConfiguration) -> Option<AppSwitchResult> {
        let matching = self
            .windows
            .get_windows()
            .into_iter()
            .filter(|window| {
                window
                    .process_name()
                    .eq_ignore_ascii_case(app.process_name())
            })
            .collect::<Vec<_>>();
        let target = match matching.first().cloned() {
            Some(window) => window,
            None if app.start_if_not_running => {
                launch(app);
                return Some(AppSwitchResult {
                    process_id: None,
                    process_path: app.process_path.clone(),
                    handle: None,
                    was_started: true,
                });
            }
            None => return None,
        };
        let current = self.windows.get_current_window();
        if current.as_ref().map(|window| window.process_id) != Some(target.process_id)
            || app.cycle_mode == CycleMode::NextApp
        {
            self.next_windows.clear();
            activate(&target);
            return Some(AppSwitchResult {
                process_id: Some(target.process_id),
                process_path: target.process_image_path.clone(),
                handle: Some(target.handle),
                was_started: false,
            });
        }
        match app.cycle_mode {
            CycleMode::Hide => hide(target.handle),
            CycleMode::NextWindow => {
                let next = self.next_window(&matching, &target);
                activate(&next);
            }
            CycleMode::NextApp => {}
        }
        None
    }
    fn next_window(
        &mut self,
        windows: &[ApplicationWindow],
        current: &ApplicationWindow,
    ) -> ApplicationWindow {
        if !self.next_windows.contains(&current.handle) {
            self.next_windows.push(current.handle);
        }
        if self.next_windows.len() >= windows.len() {
            let index = self
                .next_windows
                .iter()
                .position(|handle| *handle == current.handle)
                .unwrap_or(0);
            return windows
                .iter()
                .find(|window| {
                    window.handle == self.next_windows[(index + 1) % self.next_windows.len()]
                })
                .cloned()
                .unwrap_or_else(|| windows[0].clone());
        }
        windows
            .iter()
            .find(|window| !self.next_windows.contains(&window.handle))
            .cloned()
            .unwrap_or_else(|| windows[0].clone())
    }
}

fn launch(app: &ApplicationConfiguration) {
    if app.application_type == ApplicationType::Packaged {
        let _ = Command::new("explorer.exe")
            .args(["shell:AppsFolder\\", app.aumid.as_deref().unwrap_or("")])
            .spawn();
    } else {
        let _ = Command::new(Path::new(&app.process_path)).spawn();
    }
}
fn hide(handle: isize) {
    unsafe {
        ShowWindow(HWND(handle as *mut std::ffi::c_void), SW_MINIMIZE);
    }
}
fn activate(window: &ApplicationWindow) {
    unsafe {
        let hwnd = HWND(window.handle as *mut std::ffi::c_void);
        if IsIconic(hwnd).as_bool() {
            if window.process_id == 0 {
                ShowWindow(hwnd, SW_RESTORE);
            } else {
                SwitchToThisWindow(hwnd, true);
            }
        }
        SetForegroundWindow(hwnd);
        if GetForegroundWindow() != hwnd {
            let target_thread = GetWindowThreadProcessId(hwnd, None);
            let foreground_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
            if target_thread != foreground_thread {
                AttachThreadInput(target_thread, foreground_thread, true);
                SetForegroundWindow(hwnd);
                AttachThreadInput(target_thread, foreground_thread, false);
            }
        }
    }
}
