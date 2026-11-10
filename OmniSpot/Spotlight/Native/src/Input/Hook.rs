use super::dynamic_mode_service::DynamicModeService;
use super::key_state_machine::{timestamp, KeyStateMachine, KeyTransition};
use super::keyboard_input::{self, D0, D1};
use super::peeker::Peeker;
use crate::configuration::configuration::Configuration;
use crate::runtime::{file_name, RuntimeError};
use crate::stats::stats_event::StatsEvent;
use crate::switcher::{AppSwitchResult, Switcher};
use crate::window_discovery::equal_ignore_case;
use crate::window_discovery::i_app_name_resolver::IAppNameResolver;
use crate::window_discovery::i_window_enumerator::IWindowEnumerator;
use std::collections::HashSet;

pub(crate) struct KeyboardHookEvent {
    pub key: u32,
    pub is_key_down: bool,
    pub flags: u32,
    pub suppress_key_press: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Warning {
    Elevated,
    KeyEventsStealing,
}

// The existing C# UI/services are not connected to this port yet. A host must route
// keyboard, timer and dispatcher callbacks to the corresponding Hook methods.
pub(crate) trait HookHost {
    fn subscribe_keyboard(&mut self) -> Result<(), RuntimeError>;
    fn unsubscribe_keyboard(&mut self);
    fn dispose_keyboard(&mut self);
    fn configure_overlay_timer(&mut self, timeout_ms: i32);
    fn start_overlay_timer(&mut self);
    fn cancel_overlay_timer(&mut self);
    fn overlay_is_visible(&self) -> bool;
    fn show_overlay(
        &mut self,
        applications: &[crate::configuration::configuration::ApplicationConfiguration],
        dynamic: bool,
    );
    fn hide_overlay(&mut self);
    fn show_warning(&mut self, warning: Warning);
    fn toggle_spotlight(&mut self);
    fn enqueue_stats(&mut self, event: StatsEvent);
    // Run IProcessInspector.WaitForPotentialElevation off the UI thread and dispatch
    // potential_elevation_detected back to this Hook only when it returns true.
    fn monitor_potential_elevation(&mut self, process_path: &str);
    fn log_error(&mut self, error: &RuntimeError);
}

pub(crate) struct Hook<E: IWindowEnumerator, R: IAppNameResolver, H: HookHost> {
    pub switcher: Switcher<E>,
    pub peeker: Peeker,
    pub window_enumerator: E,
    pub dynamic_mode_service: DynamicModeService<E, R>,
    pub host: H,
    state_machine: KeyStateMachine,
    config: Option<Configuration>,
    overlay_preview_enabled: Option<bool>,
    suppressed_letter_keys: HashSet<u32>,
    suppressed_digit_keys: HashSet<u32>,
    passthrough_letter_keys: HashSet<u32>,
    previous_letter_up_tick: Option<i64>,
}

impl<E: IWindowEnumerator, R: IAppNameResolver, H: HookHost> Hook<E, R, H> {
    pub fn new(
        switcher: Switcher<E>,
        peeker: Peeker,
        window_enumerator: E,
        dynamic_mode_service: DynamicModeService<E, R>,
        host: H,
    ) -> Self {
        Self {
            switcher,
            peeker,
            window_enumerator,
            dynamic_mode_service,
            host,
            state_machine: KeyStateMachine::default(),
            config: None,
            overlay_preview_enabled: None,
            suppressed_letter_keys: HashSet::new(),
            suppressed_digit_keys: HashSet::new(),
            passthrough_letter_keys: HashSet::new(),
            previous_letter_up_tick: None,
        }
    }

    pub fn start(&mut self, config: Configuration) -> Result<(), RuntimeError> {
        self.configure(&config);
        self.config = Some(config);
        self.host.subscribe_keyboard()
    }

    pub fn stop(&mut self) {
        self.host.unsubscribe_keyboard();
    }

    pub fn update_configuration(&mut self, config: Configuration) {
        self.overlay_preview_enabled = None;
        self.configure(&config);
        self.config = Some(config);
        self.reset_modifier_state();
    }

    fn configure(&mut self, config: &Configuration) {
        self.state_machine.configure(
            config.modifier,
            config.spotlight_modifier,
            config.spotlight_enabled,
        );
        self.host
            .configure_overlay_timer(config.overlay_show_delay_ms);
    }

    pub fn set_overlay_preview_enabled(&mut self, enabled: bool) {
        self.overlay_preview_enabled = Some(enabled);
        if !enabled {
            self.host.cancel_overlay_timer();
            self.host.hide_overlay();
        }
    }

    pub fn keyboard_pressed(&mut self, event: &mut KeyboardHookEvent, culture: &str) {
        if let Err(error) = self.handle_keyboard_pressed(event, culture) {
            self.host.log_error(&error);
        }
    }

    fn handle_keyboard_pressed(
        &mut self,
        event: &mut KeyboardHookEvent,
        culture: &str,
    ) -> Result<(), RuntimeError> {
        let modifier = self
            .config
            .as_ref()
            .ok_or_else(|| RuntimeError::invalid("Hook configuration is missing"))?
            .modifier;
        if event.flags & 0x10 != 0 {
            return Ok(());
        }
        if event.is_key_down {
            if self.state_machine.is_modifier_held()
                && !keyboard_input::is_key_physically_down(modifier)
            {
                self.reset_modifier_state();
            }
            self.handle_key_down(event, culture)
        } else {
            self.handle_key_up(event)
        }
    }

    fn handle_key_down(
        &mut self,
        event: &mut KeyboardHookEvent,
        culture: &str,
    ) -> Result<(), RuntimeError> {
        use KeyTransition::*;
        match self.state_machine.process_key_down(event.key) {
            ModifierPressed {
                is_first_press,
                has_side_effect,
            } => {
                if has_side_effect {
                    event.suppress_key_press = true;
                }
                if is_first_press && self.is_overlay_enabled() {
                    self.host.start_overlay_timer();
                }
            }
            LetterKeyPressed(letter) => {
                if !keyboard_input::is_key_physically_down(self.config.as_ref().unwrap().modifier) {
                    self.reset_modifier_state();
                } else {
                    self.handle_letter_pressed(event, letter, culture)?;
                }
            }
            DigitKeyPressed(digit) => {
                if !keyboard_input::is_key_physically_down(self.config.as_ref().unwrap().modifier) {
                    self.reset_modifier_state();
                } else {
                    self.handle_digit_pressed(event, digit)?;
                }
            }
            SpotlightShortcutPressed => {
                if !keyboard_input::is_key_physically_down(
                    self.config.as_ref().unwrap().spotlight_modifier,
                ) {
                    self.reset_modifier_state();
                } else {
                    event.suppress_key_press = true;
                    self.host.toggle_spotlight();
                }
            }
            AltTabSwitched(count) => self.host.enqueue_stats(StatsEvent::AltTabEvent(count)),
            UnrelatedKeyReset => self.reset_modifier_state(),
            _ => {}
        }
        Ok(())
    }

    fn handle_key_up(&mut self, event: &mut KeyboardHookEvent) -> Result<(), RuntimeError> {
        use KeyTransition::*;
        let letter_suppressed = self.suppressed_letter_keys.remove(&event.key);
        let digit_suppressed = self.suppressed_digit_keys.remove(&event.key);
        let passthrough = self.passthrough_letter_keys.remove(&event.key);
        if letter_suppressed || digit_suppressed {
            event.suppress_key_press = true;
            if self.state_machine.is_modifier_held() {
                self.previous_letter_up_tick = Some(timestamp());
            }
            self.finish_peek();
            return Ok(());
        }
        if passthrough {
            event.suppress_key_press = true;
            keyboard_input::send_synthetic_combination(
                self.config.as_ref().unwrap().modifier,
                event.key,
            );
            return Ok(());
        }
        let was_overlay_visible = self.host.overlay_is_visible();
        match self.state_machine.process_key_up(event.key) {
            ModifierReleasedClean {
                held_duration_ms,
                has_side_effect,
            } => {
                self.host.cancel_overlay_timer();
                self.host.hide_overlay();
                self.previous_letter_up_tick = None;
                if has_side_effect {
                    event.suppress_key_press = true;
                    if !was_overlay_visible && held_duration_ms <= 200 {
                        keyboard_input::send_synthetic_key_down_up(event.key);
                    }
                }
            }
            ModifierReleasedAfterAction { has_side_effect } => {
                self.host.cancel_overlay_timer();
                self.host.hide_overlay();
                self.previous_letter_up_tick = None;
                if has_side_effect {
                    event.suppress_key_press = true;
                }
                self.finish_peek();
            }
            AltTabSwitched(count) => self.host.enqueue_stats(StatsEvent::AltTabEvent(count)),
            _ => {}
        }
        Ok(())
    }

    fn handle_letter_pressed(
        &mut self,
        event: &mut KeyboardHookEvent,
        letter: u32,
        culture: &str,
    ) -> Result<(), RuntimeError> {
        let config = self.config.as_ref().unwrap();
        let mut matching_apps: Vec<_> = config
            .applications
            .iter()
            .filter(|app| app.key == letter)
            .cloned()
            .collect();
        let mut is_dynamic = false;
        if matching_apps.is_empty() && config.dynamic_mode_enabled {
            matching_apps = self
                .dynamic_mode_service
                .get_apps_for_key(letter, &config.applications);
            is_dynamic = true;
        }
        if !matching_apps.is_empty() {
            event.suppress_key_press = true;
            if self.suppressed_letter_keys.insert(letter) {
                let letter_down_tick = timestamp();
                let current_window = self.window_enumerator.get_current_window();
                let result = self.switcher.execute(&matching_apps, culture)?;
                let stealing = result.as_ref().is_some_and(|result| {
                    equal_ignore_case(&result.process_name, "WindowsSandboxRemoteSession.exe")
                });
                if let Some(result) = &result {
                    if config.stats_enabled && !result.was_started {
                        self.host.enqueue_stats(StatsEvent::SwitchEvent {
                            process_name: result.process_name.clone(),
                            process_id: result.process_id,
                            process_path: result.process_path.clone(),
                            total_choices: self.window_enumerator.get_total_choices_count() as i32,
                            modifier_down_tick: self.state_machine.modifier_pressed_at_tick,
                            letter_down_tick,
                            previous_letter_up_tick: self.previous_letter_up_tick,
                            is_dynamic,
                            trigger_key: letter,
                        });
                    }
                    if config.peek_enabled && !result.was_started && !stealing {
                        if let Some(previous) = current_window
                            .filter(|window| Some(window.process_id) != result.process_id)
                        {
                            self.peeker.arm(previous, result.clone(), is_dynamic)?;
                            if !self.host.overlay_is_visible() {
                                self.host.cancel_overlay_timer();
                            }
                        }
                    }
                }
                if result.as_ref().is_some_and(|result| result.needs_elevation) {
                    self.reset_modifier_state();
                    self.host.show_warning(Warning::Elevated);
                } else if stealing {
                    self.reset_modifier_state();
                    self.host.show_warning(Warning::KeyEventsStealing);
                } else {
                    self.refresh_or_hide_overlay();
                    if let Some(result) = result.filter(|result| result.was_started) {
                        self.monitor_potential_elevation(result);
                    }
                }
            }
        } else if self.state_machine.configured_modifier_has_side_effect() {
            event.suppress_key_press = true;
            self.passthrough_letter_keys.insert(letter);
        }
        Ok(())
    }

    fn monitor_potential_elevation(&mut self, result: AppSwitchResult) {
        self.host.monitor_potential_elevation(&result.process_path);
    }

    pub fn potential_elevation_detected(&mut self) {
        self.reset_modifier_state();
        self.host.show_warning(Warning::Elevated);
    }

    fn handle_digit_pressed(
        &mut self,
        event: &mut KeyboardHookEvent,
        digit: u32,
    ) -> Result<(), RuntimeError> {
        let digit_down_tick = timestamp();
        let index = digit_key_to_index(digit);
        let config = self.config.as_ref().unwrap();
        let (switched, window) = self
            .switcher
            .switch_to_window_by_index(&config.applications, index)?;
        if switched {
            event.suppress_key_press = true;
            self.suppressed_digit_keys.insert(digit);
            if config.stats_enabled {
                if let Some(window) = window {
                    self.host.enqueue_stats(StatsEvent::SwitchEvent {
                        process_name: file_name(&window.process_image_path).to_string(),
                        process_id: Some(window.process_id),
                        process_path: window.process_image_path,
                        total_choices: self.window_enumerator.get_total_choices_count() as i32,
                        modifier_down_tick: self.state_machine.modifier_pressed_at_tick,
                        letter_down_tick: digit_down_tick,
                        previous_letter_up_tick: self.previous_letter_up_tick,
                        is_dynamic: false,
                        trigger_key: digit,
                    });
                }
            }
            self.refresh_or_hide_overlay();
        }
        Ok(())
    }

    fn finish_peek(&mut self) {
        if let Some(result) = self.peeker.try_finish() {
            if self
                .config
                .as_ref()
                .is_some_and(|config| config.stats_enabled)
            {
                self.host.enqueue_stats(StatsEvent::PeekEvent {
                    target_process_name: result.target_process_name,
                    target_process_path: result.target_process_path,
                    arm_tick: result.armed_at_tick,
                    finish_tick: timestamp(),
                    is_dynamic: result.is_dynamic,
                });
            }
            self.switcher.activate_window(&result.previous_window);
            if result.target_was_minimized {
                self.switcher.hide_window(result.target_handle);
            }
        }
    }

    fn refresh_or_hide_overlay(&mut self) {
        let config = self.config.as_ref().unwrap();
        if self.is_overlay_enabled()
            && config.overlay_keep_open_while_modifier_held
            && self.host.overlay_is_visible()
        {
            self.host
                .show_overlay(&config.applications, config.dynamic_mode_enabled);
        } else {
            self.host.hide_overlay();
        }
    }

    pub fn show_overlay_if_enabled(&mut self) {
        if self.is_overlay_enabled() && self.state_machine.is_modifier_held() {
            if let Some(config) = &self.config {
                self.host
                    .show_overlay(&config.applications, config.dynamic_mode_enabled);
            }
        }
    }

    fn is_overlay_enabled(&self) -> bool {
        self.overlay_preview_enabled
            .or_else(|| self.config.as_ref().map(|config| config.overlay_enabled))
            == Some(true)
    }

    fn reset_modifier_state(&mut self) {
        self.state_machine.reset();
        self.suppressed_letter_keys.clear();
        self.suppressed_digit_keys.clear();
        self.passthrough_letter_keys.clear();
        self.previous_letter_up_tick = None;
        self.peeker.cancel();
        self.host.cancel_overlay_timer();
        self.host.hide_overlay();
    }
}

impl<E: IWindowEnumerator, R: IAppNameResolver, H: HookHost> Drop for Hook<E, R, H> {
    fn drop(&mut self) {
        self.stop();
        self.host.dispose_keyboard();
    }
}

fn digit_key_to_index(key: u32) -> i32 {
    if key == D0 {
        9
    } else {
        key as i32 - D1 as i32
    }
}

#[cfg(test)]
#[path = "../tests/Input/HookTests.rs"]
mod tests;
