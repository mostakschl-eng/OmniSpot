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
use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU32, Ordering},
        mpsc::Sender,
    },
    time::{Duration, Instant},
};
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::SetForegroundWindow};

pub struct InputController {
    configuration: Arc<RwLock<Configuration>>,
    state_machine: KeyStateMachine,
    switcher: Switcher,
    suppressed_letters: HashSet<Key>,
    suppressed_digits: HashSet<Key>,
    passthrough_letters: HashSet<Key>,
    events: Option<Sender<InputEvent>>,
    action_started_at: Option<Instant>,
    overlay_cancel: Option<Sender<()>>,
    peeker: Peeker,
    app_registry: AppRegistryCache,
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
}

impl InputController {
    pub fn new(configuration: Configuration) -> Arc<Mutex<Self>> {
        Self::new_with_events(configuration, None)
    }

    pub fn new_with_events(
        configuration: Configuration,
        events: Option<Sender<InputEvent>>,
    ) -> Arc<Mutex<Self>> {
        let mut state_machine = KeyStateMachine::default();
        state_machine.configure(
            configuration.modifier,
            configuration.spotlight_modifier,
            configuration.spotlight_enabled,
        );
        Arc::new(Mutex::new(Self {
            configuration: Arc::new(RwLock::new(configuration)),
            state_machine,
            switcher: Switcher::default(),
            suppressed_letters: HashSet::new(),
            suppressed_digits: HashSet::new(),
            passthrough_letters: HashSet::new(),
            events,
            action_started_at: None,
            overlay_cancel: None,
            peeker: Peeker::default(),
            app_registry: AppRegistryCache::for_user(),
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
        self.reset();
    }
    pub fn handle_event(&mut self, key: Key, is_down: bool, injected: bool) -> bool {
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
                    let _ = events.send(InputEvent::OpenSpotlight);
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
                    let _ = events.send(InputEvent::AltTab { count: nav_count });
                }
                false
            }
            KeyTransition::ModifierReleasedClean {
                has_side_effect: true,
                ..
            } => {
                self.cancel_overlay();
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
            let choices = self.switcher.windows.get_cached_windows().len() as i32;
            self.action_started_at = Some(Instant::now());
            let previous_handle = self
                .switcher
                .windows
                .get_current_window()
                .map(|window| window.handle);
            if let Some(result) = self.switcher.execute(&matching) {
                if config.peek_enabled {
                    if let (Some(previous_handle), Some(target_handle)) =
                        (previous_handle, result.handle)
                    {
                        if previous_handle != target_handle {
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
                    }
                }
                if let Some(events) = &self.events {
                    let duration_ms = self
                        .action_started_at
                        .take()
                        .map(|started| started.elapsed().as_millis() as i32)
                        .unwrap_or_default();
                    let _ = events.send(InputEvent::Switch {
                        process: result.process_path,
                        duration_ms,
                        choices,
                        dynamic,
                        key: letter,
                    });
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
        let mut windows = self.switcher.windows.get_windows();
        windows.sort_by_key(|window| window.handle);
        if let Some(window) = windows.get(index) {
            self.suppressed_digits.insert(digit);
            activate_window(window.handle);
            true
        } else {
            false
        }
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
            let _ = events.send(InputEvent::Peek {
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
        if !config.overlay_enabled {
            return;
        }
        self.cancel_overlay();
        let (sender, receiver) = std::sync::mpsc::channel();
        let events = self.events.clone();
        let delay = Duration::from_millis(config.overlay_show_delay_ms as u64);
        std::thread::spawn(move || {
            if receiver.recv_timeout(delay).is_err() {
                if let Some(events) = events {
                    let _ = events.send(InputEvent::OverlayShow);
                }
            }
        });
        self.overlay_cancel = Some(sender);
    }

    fn cancel_overlay(&mut self) {
        if let Some(sender) = self.overlay_cancel.take() {
            let _ = sender.send(());
            if let Some(events) = &self.events {
                let _ = events.send(InputEvent::OverlayHide);
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
    pub fn stop(&self) {
        self.hook.stop();
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
        SetForegroundWindow(HWND(handle as *mut std::ffi::c_void));
    }
}
