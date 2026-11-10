use crate::configuration::application_type::ApplicationType;
use crate::configuration::configuration::{ApplicationConfiguration, CycleMode};
use crate::runtime::{file_name, start_process, RuntimeError};
use crate::window_discovery;
use crate::window_discovery::application_window::ApplicationWindow;
use crate::window_discovery::i_window_enumerator::IWindowEnumerator;
use crate::window_discovery::window_enumerator::WindowEnumerator;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::AttachThreadInput;
use windows::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_MOUSE};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Debug)]
pub(crate) struct AppSwitchResult {
    pub process_id: Option<u32>,
    pub process_path: String,
    pub handle: i64,
    pub state: u32,
    pub needs_elevation: bool,
    pub was_started: bool,
    pub process_name: String,
}

impl AppSwitchResult {
    pub fn from_application_window(window: &ApplicationWindow) -> Self {
        Self {
            process_id: Some(window.process_id),
            process_path: window.process_image_path.clone(),
            handle: window.handle,
            state: SW_SHOW.0 as u32,
            needs_elevation: window.needs_elevation,
            was_started: false,
            process_name: file_name(&window.process_image_path).to_string(),
        }
    }

    pub fn from_started(process_path: String) -> Self {
        let process_name = file_name(&process_path).to_string();
        Self {
            process_id: None,
            process_path,
            handle: 0,
            state: SW_SHOW.0 as u32,
            needs_elevation: false,
            was_started: true,
            process_name,
        }
    }
}

pub(crate) struct Switcher<E: IWindowEnumerator = WindowEnumerator> {
    window_enumerator: E,
    next_windows: Vec<i64>,
}

impl<E: IWindowEnumerator> Switcher<E> {
    pub fn new(window_enumerator: E) -> Self {
        Self {
            window_enumerator,
            next_windows: Vec::new(),
        }
    }

    pub fn execute(
        &mut self,
        app_group: &[ApplicationConfiguration],
        culture: &str,
    ) -> Result<Option<AppSwitchResult>, RuntimeError> {
        if app_group.len() == 1 {
            return self.execute_application(&app_group[0], culture);
        }
        let current = self.window_enumerator.get_current_window();
        let current_index = current.as_ref().and_then(|window| {
            app_group.iter().position(|app| {
                culture_ends_with(&window.process_image_path, app.process_name(), culture)
            })
        });
        if app_group.is_empty() {
            return Err(RuntimeError::invalid("The application group is empty"));
        }
        let next = current_index.map_or(0, |index| (index + 1) % app_group.len());
        window_discovery::log(
            1,
            format!(
                "Cycling app group: current index {}, next app {}",
                current_index.map_or(-1, |i| i as i32),
                app_group[next].process_name()
            ),
        );
        self.execute_application(&app_group[next], culture)
    }

    pub fn switch_to_window_by_index(
        &self,
        applications: &[ApplicationConfiguration],
        index: i32,
    ) -> Result<(bool, Option<ApplicationWindow>), RuntimeError> {
        let all = self.window_enumerator.get_windows();
        let focused = self
            .window_enumerator
            .get_focused_app_windows(&all, applications);
        let Some(selected) = usize::try_from(index)
            .ok()
            .and_then(|i| focused.all_windows.get(i))
        else {
            return Ok((false, None));
        };
        if focused
            .focused_window
            .as_ref()
            .is_some_and(|current| current.handle == selected.handle)
        {
            return Ok((true, None));
        }
        let current = focused
            .focused_window
            .as_ref()
            .ok_or_else(|| RuntimeError::invalid("The focused window is missing"))?;
        let name = window_discovery::get_product_name(&current.process_image_path)
            .map_err(RuntimeError::from)?
            .unwrap_or_else(|| {
                let name = file_name(&current.process_image_path);
                name.rsplit_once('.')
                    .map_or(name, |(stem, _)| stem)
                    .to_string()
            });
        window_discovery::log(1, format!("Switching to window #{} of {name}", index + 1));
        self.activate_window(selected);
        Ok((true, Some(selected.clone())))
    }

    fn execute_application(
        &mut self,
        app: &ApplicationConfiguration,
        culture: &str,
    ) -> Result<Option<AppSwitchResult>, RuntimeError> {
        let matching: Vec<_> = self
            .window_enumerator
            .get_windows()
            .into_iter()
            .filter(|window| {
                culture_ends_with(&window.process_image_path, app.process_name(), culture)
            })
            .collect();
        let Some(window) = matching.first() else {
            if app.start_if_not_running {
                if app.application_type == ApplicationType::Packaged {
                    start_process(
                        "explorer.exe",
                        Some(&format!(
                            "shell:AppsFolder\\{}",
                            app.aumid.as_deref().unwrap_or_default()
                        )),
                    )?;
                } else {
                    start_process(&app.process_path, None)?;
                }
                window_discovery::log(2, format!("Starting {}", app.process_name()));
                return Ok(Some(AppSwitchResult::from_started(
                    app.process_path.clone(),
                )));
            }
            window_discovery::warn(format!("{} process not found", app.process_name()));
            return Ok(None);
        };
        let current = self.window_enumerator.get_current_window();
        if current.as_ref().map(|current| current.process_id) != Some(window.process_id)
            || app.cycle_mode == CycleMode::NextApp
        {
            self.next_windows.clear();
            window_discovery::log(1, format!("Switching to {}", app.process_name()));
            self.activate_window(window);
            return Ok(Some(AppSwitchResult::from_application_window(window)));
        }
        match app.cycle_mode {
            CycleMode::Hide => self.hide_window(window.handle),
            CycleMode::NextWindow => {
                let next = self.get_next_window(&matching, current.as_ref().unwrap())?;
                self.activate_window(&next);
            }
            CycleMode::NextApp => {}
        }
        Ok(None)
    }

    pub fn open(&self, app: &ApplicationConfiguration, culture: &str) -> Result<(), RuntimeError> {
        let window = self
            .window_enumerator
            .get_windows()
            .into_iter()
            .find(|window| {
                culture_ends_with(&window.process_image_path, app.process_name(), culture)
            });
        if let Some(window) = window {
            self.activate_window(&window);
        } else if app.application_type == ApplicationType::Packaged {
            if let Some(aumid) = app.aumid.as_ref().filter(|aumid| !aumid.trim().is_empty()) {
                start_process("explorer.exe", Some(&format!("shell:AppsFolder\\{aumid}")))?;
            } else {
                window_discovery::warn(format!(
                    "Cannot start packaged app {} without an AUMID",
                    app.process_name()
                ));
            }
        } else {
            start_process(&app.process_path, None)?;
        }
        Ok(())
    }

    fn get_next_window(
        &mut self,
        matching: &[ApplicationWindow],
        current: &ApplicationWindow,
    ) -> Result<ApplicationWindow, RuntimeError> {
        self.window_enumerator.log_windows(0, matching);
        if !self.next_windows.contains(&current.handle) {
            self.next_windows.push(current.handle);
        }
        let next = if matching
            .iter()
            .all(|window| self.next_windows.contains(&window.handle))
        {
            let index = self
                .next_windows
                .iter()
                .position(|&handle| handle == current.handle)
                .unwrap();
            let handle = self.next_windows[(index + 1) % self.next_windows.len()];
            matching.iter().find(|window| window.handle == handle)
        } else {
            matching
                .iter()
                .find(|window| !self.next_windows.contains(&window.handle))
        };
        next.cloned().ok_or_else(|| {
            RuntimeError::invalid("The next window is no longer in the matching window list")
        })
    }

    // Border colour and animation stay in the C# UI implementation.
    pub fn activate_window(&self, window: &ApplicationWindow) {
        let hwnd = HWND(window.handle as isize as *mut _);
        unsafe {
            if IsIconic(hwnd).as_bool() {
                if window.needs_elevation {
                    SwitchToThisWindow(hwnd, true);
                } else {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
            }
            let input = INPUT {
                r#type: INPUT_MOUSE,
                ..Default::default()
            };
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
            let _ = SetForegroundWindow(hwnd);
            let foreground = GetForegroundWindow();
            if foreground != hwnd {
                let target_thread = GetWindowThreadProcessId(hwnd, None);
                let foreground_thread = GetWindowThreadProcessId(foreground, None);
                if target_thread != foreground_thread {
                    let _ = AttachThreadInput(target_thread, foreground_thread, true);
                    let _ = SetForegroundWindow(hwnd);
                    let _ = AttachThreadInput(target_thread, foreground_thread, false);
                } else {
                    let _ = SetForegroundWindow(hwnd);
                }
            }
        }
    }

    pub fn hide_window(&self, handle: i64) {
        unsafe {
            let _ = ShowWindow(HWND(handle as isize as *mut _), SW_MINIMIZE);
        }
    }
}

fn culture_ends_with(value: &str, suffix: &str, culture: &str) -> bool {
    window_discovery::process_name_matcher::ends_with(value, suffix, culture)
}

#[cfg(test)]
#[path = "tests/SwitcherTests.rs"]
mod tests;
