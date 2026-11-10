use super::keyboard_input::*;
use std::collections::HashSet;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum State {
    #[default]
    Idle,
    ModifierHeld,
    ModifierHeldWithActionKey,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum KeyTransition {
    ModifierPressed {
        is_first_press: bool,
        has_side_effect: bool,
    },
    ModifierReleasedClean {
        held_duration_ms: i64,
        has_side_effect: bool,
    },
    ModifierReleasedAfterAction {
        has_side_effect: bool,
    },
    LetterKeyPressed(u32),
    DigitKeyPressed(u32),
    UnrelatedKeyReset,
    AltTabSwitched(i32),
    SpotlightShortcutPressed,
    NoOp,
}

pub(crate) struct KeyStateMachine {
    state: State,
    configured_modifier: u32,
    spotlight_modifier: u32,
    spotlight_enabled: bool,
    held_keyboard_modifiers: HashSet<u32>,
    alt_held: bool,
    alt_tab_active: bool,
    alt_tab_is_sticky: bool,
    alt_nav_count: i32,
    pub modifier_pressed_at_tick: i64,
}

impl Default for KeyStateMachine {
    fn default() -> Self {
        Self {
            state: State::Idle,
            configured_modifier: NONE,
            spotlight_modifier: LEFT_ALT,
            spotlight_enabled: true,
            held_keyboard_modifiers: HashSet::new(),
            alt_held: false,
            alt_tab_active: false,
            alt_tab_is_sticky: false,
            alt_nav_count: 0,
            modifier_pressed_at_tick: 0,
        }
    }
}

impl KeyStateMachine {
    pub fn is_modifier_held(&self) -> bool {
        self.state != State::Idle
    }

    pub fn configured_modifier_has_side_effect(&self) -> bool {
        matches!(
            self.configured_modifier,
            LWIN | RWIN | LEFT_ALT | RIGHT_ALT | APPS | CAPITAL
        )
    }

    pub fn process_key_down(&mut self, key: u32) -> KeyTransition {
        use KeyTransition::*;
        if matches!(
            key,
            LEFT_CTRL | RIGHT_CTRL | LEFT_ALT | RIGHT_ALT | LEFT_SHIFT | RIGHT_SHIFT | LWIN | RWIN
        ) {
            self.held_keyboard_modifiers.insert(key);
        }
        if self.spotlight_enabled && key == SPACE && self.is_spotlight_modifier_held() {
            self.state = State::ModifierHeldWithActionKey;
            return SpotlightShortcutPressed;
        }
        if key != self.configured_modifier {
            if let Some(transition) = self.handle_alt_tab_tracking(key) {
                return transition;
            }
        }
        if key == self.configured_modifier {
            let has_side_effect = self.configured_modifier_has_side_effect();
            if self.state != State::Idle {
                return ModifierPressed {
                    is_first_press: false,
                    has_side_effect,
                };
            }
            self.state = State::ModifierHeld;
            self.modifier_pressed_at_tick = timestamp();
            return ModifierPressed {
                is_first_press: true,
                has_side_effect,
            };
        }
        if self.state == State::Idle {
            return NoOp;
        }
        if (A..=Z).contains(&key) || (D0..=D9).contains(&key) {
            if self
                .held_keyboard_modifiers
                .iter()
                .any(|&held| held != self.configured_modifier)
            {
                return NoOp;
            }
            self.state = State::ModifierHeldWithActionKey;
            return if (A..=Z).contains(&key) {
                LetterKeyPressed(key)
            } else {
                DigitKeyPressed(key)
            };
        }
        self.state = State::Idle;
        UnrelatedKeyReset
    }

    fn handle_alt_tab_tracking(&mut self, key: u32) -> Option<KeyTransition> {
        use KeyTransition::*;
        if matches!(key, LEFT_ALT | RIGHT_ALT) {
            self.alt_held = true;
            self.alt_tab_active = false;
            self.alt_tab_is_sticky = key == RIGHT_ALT;
            self.alt_nav_count = 0;
        } else if key == TAB && self.alt_held {
            self.alt_tab_active = true;
            self.alt_nav_count += 1;
            return Some(NoOp);
        } else if self.alt_tab_active && matches!(key, LEFT | RIGHT | UP | DOWN) {
            self.alt_nav_count += 1;
            return Some(NoOp);
        } else if key == RETURN && self.alt_tab_active && self.alt_tab_is_sticky {
            let count = self.alt_nav_count;
            self.reset_alt_tab_state();
            return Some(AltTabSwitched(count));
        } else if key == ESCAPE && self.alt_tab_active {
            self.reset_alt_tab_state();
        }
        None
    }

    pub fn process_key_up(&mut self, key: u32) -> KeyTransition {
        use KeyTransition::*;
        self.held_keyboard_modifiers.remove(&key);
        if key != self.configured_modifier {
            if key == LEFT_ALT && self.alt_held {
                let had_nav = self.alt_tab_active && !self.alt_tab_is_sticky;
                let count = self.alt_nav_count;
                self.reset_alt_tab_state();
                return if had_nav { AltTabSwitched(count) } else { NoOp };
            }
            if key == RIGHT_ALT {
                self.alt_held = false;
            }
            return NoOp;
        }
        let previous = self.state;
        self.state = State::Idle;
        self.held_keyboard_modifiers.clear();
        let has_side_effect = self.configured_modifier_has_side_effect();
        match previous {
            State::ModifierHeld => ModifierReleasedClean {
                held_duration_ms: elapsed_ms(self.modifier_pressed_at_tick),
                has_side_effect,
            },
            State::ModifierHeldWithActionKey => ModifierReleasedAfterAction { has_side_effect },
            State::Idle => NoOp,
        }
    }

    pub fn configure(&mut self, modifier: u32, spotlight_modifier: u32, spotlight_enabled: bool) {
        self.configured_modifier = modifier;
        self.spotlight_modifier = spotlight_modifier;
        self.spotlight_enabled = spotlight_enabled;
        self.reset();
    }

    pub fn reset(&mut self) {
        self.state = State::Idle;
        self.held_keyboard_modifiers.clear();
        self.reset_alt_tab_state();
    }

    fn reset_alt_tab_state(&mut self) {
        self.alt_held = false;
        self.alt_tab_active = false;
        self.alt_tab_is_sticky = false;
        self.alt_nav_count = 0;
    }

    fn is_spotlight_modifier_held(&self) -> bool {
        let family: &[u32] = match self.spotlight_modifier {
            LEFT_ALT | RIGHT_ALT => &[LEFT_ALT, RIGHT_ALT],
            LEFT_CTRL | RIGHT_CTRL => &[LEFT_CTRL, RIGHT_CTRL],
            LEFT_SHIFT | RIGHT_SHIFT => &[LEFT_SHIFT, RIGHT_SHIFT],
            LWIN | RWIN => &[LWIN, RWIN],
            _ => std::slice::from_ref(&self.spotlight_modifier),
        };
        family
            .iter()
            .any(|key| self.held_keyboard_modifiers.contains(key))
    }
}

pub(super) fn timestamp() -> i64 {
    let mut tick = 0;
    unsafe {
        QueryPerformanceCounter(&mut tick).expect("Windows performance counter unavailable");
    }
    tick
}

fn elapsed_ms(start: i64) -> i64 {
    let mut frequency = 0;
    unsafe {
        QueryPerformanceFrequency(&mut frequency)
            .expect("Windows performance frequency unavailable");
    }
    ((timestamp() - start) as f64 * (1000.0 / frequency as f64)) as i64
}

#[cfg(test)]
#[path = "../tests/Input/KeyStateMachineTests.rs"]
mod tests;
