use super::*;
use crate::configuration::app_theme_setting::AppThemeSetting;
use crate::configuration::configuration::ApplicationConfiguration;
use crate::input::keyboard_input::{A, CAPITAL, LEFT_ALT, LWIN, RIGHT_ALT, TAB};
use crate::window_discovery::packaged_apps_service::PackagedAppsService;
use crate::window_discovery::window_enumerator::WindowEnumerator;

struct Resolver;
impl IAppNameResolver for Resolver {
    fn get_dynamic_key(&self, _: &str, _: &str) -> Option<u32> {
        None
    }
}

#[derive(Default)]
struct Host {
    visible: bool,
    subscriptions: usize,
    cancellations: usize,
    timer_delay: i32,
    events: Vec<StatsEvent>,
    warnings: Vec<Warning>,
    errors: Vec<String>,
}
impl HookHost for Host {
    fn subscribe_keyboard(&mut self) -> Result<(), RuntimeError> {
        self.subscriptions += 1;
        Ok(())
    }
    fn unsubscribe_keyboard(&mut self) {
        self.subscriptions = self.subscriptions.saturating_sub(1);
    }
    fn dispose_keyboard(&mut self) {}
    fn configure_overlay_timer(&mut self, timeout_ms: i32) {
        self.timer_delay = timeout_ms;
        self.cancel_overlay_timer();
    }
    fn start_overlay_timer(&mut self) {}
    fn cancel_overlay_timer(&mut self) {
        self.cancellations += 1;
    }
    fn overlay_is_visible(&self) -> bool {
        self.visible
    }
    fn show_overlay(&mut self, _: &[ApplicationConfiguration], _: bool) {
        self.visible = true;
    }
    fn hide_overlay(&mut self) {
        self.visible = false;
    }
    fn show_warning(&mut self, warning: Warning) {
        self.warnings.push(warning);
    }
    fn toggle_spotlight(&mut self) {
        panic!("Unexpected Spotlight request");
    }
    fn enqueue_stats(&mut self, event: StatsEvent) {
        self.events.push(event);
    }
    fn monitor_potential_elevation(&mut self, _: &str) {
        panic!("Unexpected process launch");
    }
    fn log_error(&mut self, error: &RuntimeError) {
        self.errors.push(error.message.clone());
    }
}

fn config() -> Configuration {
    Configuration {
        modifier: LWIN,
        applications: Vec::new(),
        pulse_border_enabled: true,
        theme: AppThemeSetting::System,
        overlay_enabled: true,
        overlay_show_delay_ms: 300,
        overlay_keep_open_while_modifier_held: true,
        peek_enabled: true,
        dynamic_mode_enabled: false,
        stats_enabled: false,
        spotlight_enabled: true,
        spotlight_modifier: LEFT_ALT,
        spotlight_custom_directories: None,
    }
}

fn hook() -> Hook<WindowEnumerator, Resolver, Host> {
    Hook::new(
        Switcher::new(WindowEnumerator::default()),
        Peeker::default(),
        WindowEnumerator::default(),
        DynamicModeService {
            window_enumerator: WindowEnumerator::default(),
            app_name_resolver: Resolver,
            packaged_apps_service: PackagedAppsService::default(),
        },
        Host::default(),
    )
}

fn event(key: u32, down: bool) -> KeyboardHookEvent {
    KeyboardHookEvent {
        key,
        is_key_down: down,
        flags: 0,
        suppress_key_press: false,
    }
}

#[test]
fn digit_shortcuts_keep_the_original_one_to_zero_numbering() {
    for offset in 0..9 {
        assert_eq!(digit_key_to_index(D1 + offset), offset as i32);
    }
    assert_eq!(digit_key_to_index(D0), 9);
}

#[test]
fn injected_events_are_ignored_and_missing_configuration_is_logged() {
    let mut hook = hook();
    hook.keyboard_pressed(&mut event(A, true), "en-US");
    assert_eq!(hook.host.errors.len(), 1);
    hook.start(config()).unwrap();
    let mut injected = event(LWIN, true);
    injected.flags = 0x10;
    hook.keyboard_pressed(&mut injected, "en-US");
    assert!(!injected.suppress_key_press);
    assert!(!hook.state_machine.is_modifier_held());
    assert_eq!(hook.host.subscriptions, 1);
    assert_eq!(hook.host.timer_delay, 300);
}

#[test]
fn suppressed_key_up_is_consumed_once_and_records_previous_release() {
    let mut hook = hook();
    hook.start(config()).unwrap();
    hook.state_machine.process_key_down(LWIN);
    hook.suppressed_letter_keys.insert(A);
    let mut up = event(A, false);
    hook.handle_key_up(&mut up).unwrap();
    assert!(up.suppress_key_press);
    assert!(hook.previous_letter_up_tick.is_some());
    let mut next = event(A, false);
    hook.handle_key_up(&mut next).unwrap();
    assert!(!next.suppress_key_press);
}

#[test]
fn unbound_letter_is_deferred_for_modifiers_with_side_effects_only() {
    let mut hook = hook();
    hook.start(config()).unwrap();
    hook.state_machine.process_key_down(LWIN);
    let mut down = event(A, true);
    hook.handle_letter_pressed(&mut down, A, "en-US").unwrap();
    assert!(down.suppress_key_press);
    assert!(hook.passthrough_letter_keys.contains(&A));
}

#[test]
fn configuration_change_clears_pending_keys_and_preview_override() {
    let mut hook = hook();
    hook.start(config()).unwrap();
    hook.state_machine.process_key_down(LWIN);
    hook.suppressed_letter_keys.insert(A);
    hook.suppressed_digit_keys.insert(D1);
    hook.passthrough_letter_keys.insert(A + 1);
    hook.previous_letter_up_tick = Some(10);
    hook.set_overlay_preview_enabled(false);
    hook.update_configuration(config());
    assert!(!hook.state_machine.is_modifier_held());
    assert!(hook.suppressed_letter_keys.is_empty());
    assert!(hook.suppressed_digit_keys.is_empty());
    assert!(hook.passthrough_letter_keys.is_empty());
    assert_eq!(hook.previous_letter_up_tick, None);
    assert_eq!(hook.overlay_preview_enabled, None);
    assert!(!hook.host.visible);
}

#[test]
fn sticky_alt_tab_stats_are_enqueued_even_when_stats_are_disabled() {
    let mut hook = hook();
    hook.start(config()).unwrap();
    hook.state_machine.process_key_down(RIGHT_ALT);
    hook.state_machine.process_key_down(TAB);
    hook.state_machine.process_key_up(RIGHT_ALT);
    hook.handle_key_down(&mut event(keyboard_input::RETURN, true), "en-US")
        .unwrap();
    assert_eq!(hook.host.events, [StatsEvent::AltTabEvent(1)]);
}

#[test]
fn elevation_completion_resets_pending_input_and_shows_warning() {
    let mut hook = hook();
    let mut configuration = config();
    configuration.modifier = CAPITAL;
    hook.start(configuration).unwrap();
    hook.state_machine.process_key_down(CAPITAL);
    hook.host.visible = true;
    hook.potential_elevation_detected();
    assert!(!hook.state_machine.is_modifier_held());
    assert!(!hook.host.visible);
    assert_eq!(hook.host.warnings, [Warning::Elevated]);
}
