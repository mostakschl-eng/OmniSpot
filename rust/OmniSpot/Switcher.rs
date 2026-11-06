use crate::{
    configuration::{ApplicationConfiguration, ApplicationType, CycleMode},
    window_discovery::{ApplicationWindow, WindowEnumerator},
};
use std::{path::Path, process::Command};
use windows::Win32::{
    Foundation::HWND,
    Graphics::Dwm::{DWMWA_BORDER_COLOR, DwmSetWindowAttribute},
    System::Threading::AttachThreadInput,
    UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT, SendInput},
    UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, IsIconic, SW_MINIMIZE, SW_RESTORE,
        SetForegroundWindow, ShowWindow, SwitchToThisWindow,
    },
};

#[derive(Debug, Clone)]
pub struct AppSwitchResult {
    pub process_path: String,
    pub handle: Option<isize>,
    pub was_started: bool,
    pub needs_elevation: bool,
}

#[derive(Default)]
pub struct Switcher {
    pub windows: WindowEnumerator,
    next_windows: Vec<isize>,
}

impl Switcher {
    pub fn activate_handle(handle: isize) {
        let mut enumerator = WindowEnumerator::default();
        if let Some(window) = enumerator
            .get_all_windows()
            .into_iter()
            .find(|window| window.handle == handle)
        {
            activate(&window, false);
        }
    }

    pub fn hide_handle(handle: isize) {
        hide(handle);
    }

    pub fn execute(
        &mut self,
        group: &[ApplicationConfiguration],
        pulse_border: bool,
    ) -> Option<AppSwitchResult> {
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
            pulse_border,
        )
    }
    fn execute_one(
        &mut self,
        app: &ApplicationConfiguration,
        pulse_border: bool,
    ) -> Option<AppSwitchResult> {
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
                if launch(app).is_err() {
                    return None;
                }
                return Some(AppSwitchResult {
                    process_path: app.process_path.clone(),
                    handle: None,
                    was_started: true,
                    needs_elevation: false,
                });
            }
            None => return None,
        };
        let current = self.windows.get_current_window();
        if current.as_ref().map(|window| window.process_id) != Some(target.process_id)
            || app.cycle_mode == CycleMode::NextApp
        {
            self.next_windows.clear();
            activate(&target, pulse_border);
            return Some(AppSwitchResult {
                process_path: target.process_image_path.clone(),
                handle: Some(target.handle),
                was_started: false,
                needs_elevation: target.needs_elevation,
            });
        }
        match app.cycle_mode {
            CycleMode::Hide => hide(target.handle),
            CycleMode::NextWindow => {
                let next = self.next_window(&matching, &target);
                activate(&next, pulse_border);
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

fn launch(app: &ApplicationConfiguration) -> Result<(), std::io::Error> {
    if app.application_type == ApplicationType::Packaged {
        Command::new("explorer.exe")
            .arg(format!(
                "shell:AppsFolder\\{}",
                app.aumid.as_deref().unwrap_or("")
            ))
            .spawn()?;
    } else {
        Command::new(Path::new(&app.process_path)).spawn()?;
    }
    Ok(())
}
fn hide(handle: isize) {
    unsafe {
        let _ = ShowWindow(HWND(handle as *mut std::ffi::c_void), SW_MINIMIZE);
    }
}
fn activate(window: &ApplicationWindow, pulse_border: bool) {
    unsafe {
        let hwnd = HWND(window.handle as *mut std::ffi::c_void);
        if IsIconic(hwnd).as_bool() {
            if window.process_id == 0 {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            } else {
                SwitchToThisWindow(hwnd, true);
            }
        }
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT::default(),
            },
        };
        let _ = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        let _ = SetForegroundWindow(hwnd);
        if GetForegroundWindow() != hwnd {
            let target_thread = GetWindowThreadProcessId(hwnd, None);
            let foreground_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
            if target_thread != foreground_thread {
                let _ = AttachThreadInput(target_thread, foreground_thread, true);
                let _ = SetForegroundWindow(hwnd);
                let _ = AttachThreadInput(target_thread, foreground_thread, false);
            }
        }
    }
    if pulse_border {
        let handle = window.handle;
        std::thread::spawn(move || {
            for color in [0x00d7ff, -1, 0x00d7ff, -1] {
                unsafe {
                    let _ = DwmSetWindowAttribute(
                        HWND(handle as *mut std::ffi::c_void),
                        DWMWA_BORDER_COLOR,
                        (&color as *const i32).cast(),
                        std::mem::size_of::<i32>() as u32,
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
    }
}
