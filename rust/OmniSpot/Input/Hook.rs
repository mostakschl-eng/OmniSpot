use super::{
    KeyStateMachine, KeyTransition,
    dynamic_mode_service::DynamicModeService,
    keyboard_hook::{KeyHandler, KeyboardHook},
    keyboard_input,
    peeker::Peeker,
};
use crate::{
    configuration::{ApplicationConfiguration, Configuration, Key},
    switcher::Switcher,
    window_discovery::AppRegistryCache,
};
use async_channel::Sender as EventSender;
use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::SetForegroundWindow};

pub struct InputController {
    configuration: Arc<RwLock<Configuration>>,
    state_machine: KeyStateMachine,
    switcher: Switcher,
    suppressed_letters: HashSet<Key>,
    suppressed_digits: HashSet<Key>,
    passthrough_letters: HashSet<Key>,
    events: Option<EventSender<InputEvent>>,
    overlay_cancel: Option<std::sync::mpsc::Sender<()>>,
    peeker: Peeker,
    app_registry: AppRegistryCache,
    warning_service: crate::overlay::warning_overlay_service::WarningOverlayService,
    overlay_visible: Arc<AtomicBool>,
    overlay_preview_enabled: Option<bool>,
    previous_letter_released_at: Option<std::time::Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    OpenSpotlight,
    OpenSettings,
    Exit,
    ActivateWindow {
        handle: isize,
    },
    LaunchApplication {
        path: String,
        aumid: Option<String>,
    },
    ConfigurationUpdated(Configuration),
    Peek {
        process: String,
        duration_ms: i32,
        dynamic: bool,
    },
    Switch {
        process: String,
        duration_ms: i32,
        choices: i32,
        dynamic: bool,
        key: Key,
    },
    AltTab {
        count: i32,
    },
    OverlayShow,
    OverlayHide,
    WarningShow(crate::overlay::warning_content::WarningContent),
    WarningHide,
    DismissWarning,
    OverlayPreview(bool),
}

impl InputController {
    pub fn new_with_events(
        configuration: Configuration,
        events: Option<EventSender<InputEvent>>,
        app_registry: AppRegistryCache,
    ) -> Arc<Mutex<Self>> {
        let mut state_machine = KeyStateMachine::default();
        state_machine.configure(
            configuration.modifier,
            configuration.spotlight_modifier,
            configuration.spotlight_enabled,
        );
        let show_events = events.clone();
        let hide_events = events.clone();
        let warning_service = crate::overlay::warning_overlay_service::WarningOverlayService::new(
            move |content| {
                if let Some(events) = &show_events {
                    let _ = events.try_send(InputEvent::WarningShow(content));
                }
            },
            move || {
                if let Some(events) = &hide_events {
                    let _ = events.try_send(InputEvent::WarningHide);
                }
            },
        );
        Arc::new(Mutex::new(Self {
            configuration: Arc::new(RwLock::new(configuration)),
            state_machine,
            switcher: Switcher::default(),
            suppressed_letters: HashSet::new(),
            suppressed_digits: HashSet::new(),
            passthrough_letters: HashSet::new(),
            events,
            overlay_cancel: None,
            peeker: Peeker::default(),
            app_registry,
            warning_service,
            overlay_visible: Arc::new(AtomicBool::new(false)),
            overlay_preview_enabled: None,
            previous_letter_released_at: None,
        }))
    }
    pub fn update_configuration(&mut self, configuration: Configuration) {
        self.state_machine.configure(
            configuration.modifier,
            configuration.spotlight_modifier,
            configuration.spotlight_enabled,
        );
        *self
            .configuration
            .write()
            .expect("configuration lock poisoned") = configuration;
        self.overlay_preview_enabled = None;
        self.reset();
    }
    pub fn dismiss_warning(&mut self) {
        self.warning_service.dismiss();
    }
    pub fn set_overlay_preview_enabled(&mut self, enabled: bool) {
        self.overlay_preview_enabled = Some(enabled);
        if !enabled {
            self.cancel_overlay();
        }
    }
    pub fn handle_event(&mut self, key: Key, is_down: bool, injected: bool) -> bool {
        crate::logging::trace(format!(
            "Key event {key:?}, down={is_down}, injected={injected}"
        ));
        if injected {
            return false;
        }
        if is_down {
            self.handle_key_down(key)
        } else {
            self.handle_key_up(key)
        }
    }
    fn handle_key_down(&mut self, key: Key) -> bool {
        match self.state_machine.process_key_down(key) {
            KeyTransition::ModifierPressed {
                is_first_press,
                has_side_effect,
            } => {
                if is_first_press {
                    self.start_overlay_timer();
                }
                has_side_effect
            }
            KeyTransition::LetterKeyPressed(letter) => self.handle_letter(letter),
            KeyTransition::DigitKeyPressed(digit) => self.handle_digit(digit),
            KeyTransition::SpotlightShortcutPressed => {
                if let Some(events) = &self.events {
                    let _ = events.try_send(InputEvent::OpenSpotlight);
                }
                true
            }
            KeyTransition::UnrelatedKeyReset => {
                self.reset();
                false
            }
            _ => false,
        }
    }
    fn handle_key_up(&mut self, key: Key) -> bool {
        if self.suppressed_letters.remove(&key) || self.suppressed_digits.remove(&key) {
            self.finish_peek();
            return true;
        }
        if self.passthrough_letters.remove(&key) {
            let modifier = self
                .configuration
                .read()
                .expect("configuration lock poisoned")
                .modifier;
            return keyboard_input::send_synthetic_combination(modifier, key);
        }
        match self.state_machine.process_key_up(key) {
            KeyTransition::AltTabSwitched { nav_count } => {
                if let Some(events) = &self.events {
                    let _ = events.try_send(InputEvent::AltTab { count: nav_count });
                }
                false
            }
            KeyTransition::ModifierReleasedClean {
                held_duration_ms,
                has_side_effect: true,
            } => {
                let was_overlay_visible = self.overlay_visible.load(Ordering::SeqCst);
                self.cancel_overlay();
                if !was_overlay_visible && held_duration_ms <= 200 {
                    let _ = keyboard_input::send_synthetic_key_down_up(key);
                }
                true
            }
            KeyTransition::ModifierReleasedAfterAction {
                has_side_effect: true,
            } => {
                self.cancel_overlay();
                self.finish_peek();
                true
            }
            _ => false,
        }
    }
    fn handle_letter(&mut self, letter: Key) -> bool {
        let config = self
            .configuration
            .read()
            .expect("configuration lock poisoned")
            .clone();
        let mut matching = configured_apps(&config, letter);
        let mut dynamic = false;
        if matching.is_empty() && config.dynamic_mode_enabled {
            let windows = self.switcher.windows.get_cached_windows();
            matching = DynamicModeService::get_apps_for_key_with_registry(
                letter,
                &config.applications,
                &windows,
                &mut self.app_registry,
            );
            dynamic = !matching.is_empty();
        }
        if !matching.is_empty() {
            self.suppressed_letters.insert(letter);
            let choices = self.switcher.windows.get_total_choices_count() as i32;
            let previous_handle = self
                .switcher
                .windows
                .get_current_window()
                .map(|window| window.handle);
            if let Some(result) = self
                .switcher
                .execute(&matching, config.pulse_border_enabled)
            {
                if config.peek_enabled
                    && let (Some(previous_handle), Some(target_handle)) =
                        (previous_handle, result.handle)
                    && previous_handle != target_handle
                {
                    self.peeker.arm(
                        previous_handle,
                        target_handle,
                        result.process_path.clone(),
                        self.switcher
                            .windows
                            .get_all_windows()
                            .into_iter()
                            .find(|window| window.handle == target_handle)
                            .map(|window| window.is_minimized)
                            .unwrap_or(false),
                        dynamic,
                    );
                }
                if !result.was_started
                    && let Some(events) = &self.events
                {
                    let duration_ms = self
                        .state_machine
                        .modifier_pressed_at()
                        .map(|started| started.elapsed().as_millis() as i32)
                        .unwrap_or_default();
                    let _ = events.try_send(InputEvent::Switch {
                        process: result.process_path.clone(),
                        duration_ms,
                        choices,
                        dynamic,
                        key: letter,
                    });
                }
                if result.needs_elevation {
                    self.reset();
                    self.warning_service
                        .show(crate::overlay::warning_content::WarningContent::Elevated);
                } else if result
                    .process_path
                    .rsplit(['\\', '/'])
                    .next()
                    .is_some_and(|name| {
                        name.eq_ignore_ascii_case("WindowsSandboxRemoteSession.exe")
                    })
                {
                    self.reset();
                    self.warning_service
                        .show(crate::overlay::warning_content::WarningContent::KeyEventsStealing);
                }
            }
            return true;
        }
        if self.state_machine.configured_modifier_has_side_effect() {
            self.passthrough_letters.insert(letter);
            return true;
        }
        false
    }
    fn handle_digit(&mut self, digit: Key) -> bool {
        let config = self
            .configuration
            .read()
            .expect("configuration lock poisoned")
            .clone();
        let index = match digit {
            Key::D1 => 0,
            Key::D2 => 1,
            Key::D3 => 2,
            Key::D4 => 3,
            Key::D5 => 4,
            Key::D6 => 5,
            Key::D7 => 6,
            Key::D8 => 7,
            Key::D9 => 8,
            Key::D0 => 9,
            _ => return false,
        };
        let windows = self.switcher.windows.get_windows();
        let focused_windows = self
            .switcher
            .windows
            .get_focused_app_windows(&windows, &config.applications);
        let Some(window) = focused_windows.get(index) else {
            return false;
        };
        self.suppressed_digits.insert(digit);
        if self
            .switcher
            .windows
            .get_current_window()
            .is_some_and(|current| current.handle == window.handle)
        {
            return true;
        }
        let duration_ms = self
            .state_machine
            .modifier_pressed_at()
            .map(|started| started.elapsed().as_millis() as i32)
            .unwrap_or_default();
        activate_window(window.handle);
        if let Some(events) = &self.events {
            let _ = events.try_send(InputEvent::Switch {
                process: window.process_image_path.clone(),
                duration_ms,
                choices: self.switcher.windows.get_total_choices_count() as i32,
                dynamic: false,
                key: digit,
            });
        }
        true
    }
    fn reset(&mut self) {
        self.cancel_overlay();
        self.peeker.cancel();
        self.state_machine.reset();
        self.suppressed_letters.clear();
        self.suppressed_digits.clear();
        self.passthrough_letters.clear();
    }

    fn finish_peek(&mut self) {
        let Some(result) = self.peeker.finish() else {
            return;
        };
        Switcher::activate_handle(result.previous_handle);
        if result.target_was_minimized {
            Switcher::hide_handle(result.target_handle);
        }
        if let Some(events) = &self.events {
            let _ = events.try_send(InputEvent::Peek {
                process: result.target_process,
                duration_ms: result.armed_at.elapsed().as_millis() as i32,
                dynamic: result.dynamic,
            });
        }
    }

    fn start_overlay_timer(&mut self) {
        let config = self
            .configuration
            .read()
            .expect("configuration lock poisoned")
            .clone();
        if !self
            .overlay_preview_enabled
            .unwrap_or(config.overlay_enabled)
        {
            return;
        }
        self.cancel_overlay();
        let (sender, receiver) = std::sync::mpsc::channel();
        let events = self.events.clone();
        let overlay_visible = Arc::clone(&self.overlay_visible);
        let delay = Duration::from_millis(config.overlay_show_delay_ms as u64);
        std::thread::spawn(move || {
            if receiver.recv_timeout(delay).is_err() {
                overlay_visible.store(true, Ordering::SeqCst);
                if let Some(events) = events {
                    let _ = events.try_send(InputEvent::OverlayShow);
                }
            }
        });
        self.overlay_cancel = Some(sender);
    }

    fn cancel_overlay(&mut self) {
        self.overlay_visible.store(false, Ordering::SeqCst);
        if let Some(sender) = self.overlay_cancel.take() {
            let _ = sender.send(());
            if let Some(events) = &self.events {
                let _ = events.try_send(InputEvent::OverlayHide);
            }
        }
    }
}

pub struct InputHook {
    hook: KeyboardHook,
}
impl InputHook {
    pub fn start(controller: Arc<Mutex<InputController>>) -> Result<Self, String> {
        let handler: KeyHandler = Arc::new(move |key, down, injected| {
            controller
                .lock()
                .expect("input lock poisoned")
                .handle_event(key, down, injected)
        });
        Ok(Self {
            hook: KeyboardHook::start(handler)?,
        })
    }
    pub fn pump(&self) -> Result<(), String> {
        self.hook.pump()
    }
    pub fn spawn(controller: Arc<Mutex<InputController>>) -> InputThread {
        let thread_id = Arc::new(AtomicU32::new(0));
        let thread_id_for_worker = Arc::clone(&thread_id);
        let join = std::thread::spawn(move || {
            thread_id_for_worker.store(KeyboardHook::current_thread_id(), Ordering::Release);
            let hook = Self::start(controller)?;
            thread_id_for_worker.store(hook.hook.thread_id(), Ordering::Release);
            hook.pump()
        });
        InputThread {
            join: Some(join),
            thread_id,
        }
    }
}

pub struct InputThread {
    join: Option<std::thread::JoinHandle<Result<(), String>>>,
    thread_id: Arc<AtomicU32>,
}

impl Drop for InputThread {
    fn drop(&mut self) {
        let thread_id = self.thread_id.load(Ordering::Acquire);
        if thread_id != 0 {
            KeyboardHook::stop_thread(thread_id);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn configured_apps(config: &Configuration, key: Key) -> Vec<ApplicationConfiguration> {
    config
        .applications
        .iter()
        .filter(|app| app.key == key)
        .cloned()
        .collect()
}
fn activate_window(handle: isize) {
    unsafe {
        let _ = SetForegroundWindow(HWND(handle as *mut std::ffi::c_void));
    }
}
