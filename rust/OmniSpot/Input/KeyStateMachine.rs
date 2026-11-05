use crate::configuration::Key;
use std::{collections::HashSet, time::Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyTransition {
    ModifierPressed {
        is_first_press: bool,
        has_side_effect: bool,
    },
    ModifierReleasedClean {
        held_duration_ms: u128,
        has_side_effect: bool,
    },
    ModifierReleasedAfterAction {
        has_side_effect: bool,
    },
    LetterKeyPressed(Key),
    DigitKeyPressed(Key),
    UnrelatedKeyReset,
    AltTabSwitched {
        nav_count: i32,
    },
    SpotlightShortcutPressed,
    NoOp,
}

pub struct KeyStateMachine {
    configured_modifier: Key,
    spotlight_modifier: Key,
    spotlight_enabled: bool,
    state: State,
    pressed_at: Option<Instant>,
    held_keyboard_modifiers: HashSet<Key>,
    alt_held: bool,
    alt_tab_active: bool,
    alt_tab_sticky: bool,
    alt_nav_count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    ModifierHeld,
    ModifierHeldWithActionKey,
}

impl Default for KeyStateMachine {
    fn default() -> Self {
        Self {
            configured_modifier: Key::None,
            spotlight_modifier: Key::LeftAlt,
            spotlight_enabled: true,
            state: State::Idle,
            pressed_at: None,
            held_keyboard_modifiers: HashSet::new(),
            alt_held: false,
            alt_tab_active: false,
            alt_tab_sticky: false,
            alt_nav_count: 0,
        }
    }
}

impl KeyStateMachine {
    pub fn configure(&mut self, modifier: Key, spotlight_modifier: Key, spotlight_enabled: bool) {
        self.configured_modifier = modifier;
        self.spotlight_modifier = spotlight_modifier;
        self.spotlight_enabled = spotlight_enabled;
        self.reset();
    }
    pub fn reset(&mut self) {
        self.state = State::Idle;
        self.pressed_at = None;
        self.alt_held = false;
        self.alt_tab_active = false;
        self.alt_tab_sticky = false;
        self.alt_nav_count = 0;
    }
    pub fn is_modifier_held(&self) -> bool {
        self.state != State::Idle
    }
    pub fn configured_modifier_has_side_effect(&self) -> bool {
        has_side_effect(self.configured_modifier)
    }
    pub fn modifier_pressed_at(&self) -> Option<Instant> {
        self.pressed_at
    }

    pub fn process_key_down(&mut self, key: Key) -> KeyTransition {
        if is_keyboard_modifier(key) {
            self.held_keyboard_modifiers.insert(key);
        }
        if self.spotlight_enabled && key == Key::Space && self.is_spotlight_modifier_held() {
            return KeyTransition::SpotlightShortcutPressed;
        }
        if key != self.configured_modifier {
            if let Some(transition) = self.handle_alt_tab_down(key) {
                return transition;
            }
        }
        if key == self.configured_modifier {
            let first = self.state == State::Idle;
            if first {
                self.state = State::ModifierHeld;
                self.pressed_at = Some(Instant::now());
            }
            return KeyTransition::ModifierPressed {
                is_first_press: first,
                has_side_effect: has_side_effect(key),
            };
        }
        if self.state == State::Idle {
            return KeyTransition::NoOp;
        }
        if is_letter(key) {
            if self.has_unexpected_modifier() {
                return KeyTransition::NoOp;
            }
            self.state = State::ModifierHeldWithActionKey;
            return KeyTransition::LetterKeyPressed(key);
        }
        if key.is_digit() {
            if self.has_unexpected_modifier() {
                return KeyTransition::NoOp;
            }
            self.state = State::ModifierHeldWithActionKey;
            return KeyTransition::DigitKeyPressed(key);
        }
        self.state = State::Idle;
        KeyTransition::UnrelatedKeyReset
    }

    pub fn process_key_up(&mut self, key: Key) -> KeyTransition {
        self.held_keyboard_modifiers.remove(&key);
        if key != self.configured_modifier {
            if key == Key::LeftAlt && self.alt_held {
                let had_nav = self.alt_tab_active && !self.alt_tab_sticky;
                let count = self.alt_nav_count;
                self.reset_alt_tab();
                return if had_nav {
                    KeyTransition::AltTabSwitched { nav_count: count }
                } else {
                    KeyTransition::NoOp
                };
            }
            if key == Key::RightAlt {
                self.alt_held = false;
            }
            return KeyTransition::NoOp;
        }
        let previous = self.state;
        self.state = State::Idle;
        match previous {
            State::ModifierHeld => KeyTransition::ModifierReleasedClean {
                held_duration_ms: self
                    .pressed_at
                    .take()
                    .map(|at| at.elapsed().as_millis())
                    .unwrap_or_default(),
                has_side_effect: has_side_effect(key),
            },
            State::ModifierHeldWithActionKey => KeyTransition::ModifierReleasedAfterAction {
                has_side_effect: has_side_effect(key),
            },
            State::Idle => KeyTransition::NoOp,
        }
    }

    fn handle_alt_tab_down(&mut self, key: Key) -> Option<KeyTransition> {
        if matches!(key, Key::LeftAlt | Key::RightAlt) {
            self.alt_held = true;
            self.alt_tab_active = false;
            self.alt_tab_sticky = key == Key::RightAlt;
            self.alt_nav_count = 0;
            return None;
        }
        if key == Key::Tab && self.alt_held {
            self.alt_tab_active = true;
            self.alt_nav_count += 1;
            return Some(KeyTransition::NoOp);
        }
        if self.alt_tab_active && matches!(key, Key::Left | Key::Right | Key::Up | Key::Down) {
            self.alt_nav_count += 1;
            return Some(KeyTransition::NoOp);
        }
        if key == Key::Return && self.alt_tab_active && self.alt_tab_sticky {
            let count = self.alt_nav_count;
            self.reset_alt_tab();
            return Some(KeyTransition::AltTabSwitched { nav_count: count });
        }
        if key == Key::Escape && self.alt_tab_active {
            self.reset_alt_tab();
        }
        None
    }
    fn reset_alt_tab(&mut self) {
        self.alt_held = false;
        self.alt_tab_active = false;
        self.alt_tab_sticky = false;
        self.alt_nav_count = 0;
    }
    fn is_spotlight_modifier_held(&self) -> bool {
        match self.spotlight_modifier {
            Key::LeftAlt | Key::RightAlt => {
                self.held_keyboard_modifiers.contains(&Key::LeftAlt)
                    || self.held_keyboard_modifiers.contains(&Key::RightAlt)
            }
            Key::LeftCtrl | Key::RightCtrl => {
                self.held_keyboard_modifiers.contains(&Key::LeftCtrl)
                    || self.held_keyboard_modifiers.contains(&Key::RightCtrl)
            }
            Key::LeftShift | Key::RightShift => {
                self.held_keyboard_modifiers.contains(&Key::LeftShift)
                    || self.held_keyboard_modifiers.contains(&Key::RightShift)
            }
            Key::LWin | Key::RWin => {
                self.held_keyboard_modifiers.contains(&Key::LWin)
                    || self.held_keyboard_modifiers.contains(&Key::RWin)
            }
            _ => self
                .held_keyboard_modifiers
                .contains(&self.spotlight_modifier),
        }
    }
    fn has_unexpected_modifier(&self) -> bool {
        self.held_keyboard_modifiers
            .iter()
            .any(|key| *key != self.configured_modifier)
    }
}

fn is_letter(key: Key) -> bool {
    key.is_letter()
}
fn is_keyboard_modifier(key: Key) -> bool {
    matches!(
        key,
        Key::LeftCtrl
            | Key::RightCtrl
            | Key::LeftAlt
            | Key::RightAlt
            | Key::LeftShift
            | Key::RightShift
            | Key::LWin
            | Key::RWin
    )
}
fn has_side_effect(key: Key) -> bool {
    matches!(
        key,
        Key::LWin | Key::LeftAlt | Key::RightAlt | Key::Apps | Key::Capital
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emits_letter_transition_after_configured_modifier() {
        let mut machine = KeyStateMachine::default();
        machine.configure(Key::RightCtrl, Key::LeftAlt, true);
        assert!(matches!(
            machine.process_key_down(Key::RightCtrl),
            KeyTransition::ModifierPressed {
                is_first_press: true,
                ..
            }
        ));
        assert_eq!(
            machine.process_key_down(Key::A),
            KeyTransition::LetterKeyPressed(Key::A)
        );
        assert!(matches!(
            machine.process_key_up(Key::RightCtrl),
            KeyTransition::ModifierReleasedAfterAction { .. }
        ));
    }
    #[test]
    fn detects_spotlight_shortcut() {
        let mut machine = KeyStateMachine::default();
        machine.configure(Key::RightCtrl, Key::LeftAlt, true);
        machine.process_key_down(Key::LeftAlt);
        assert_eq!(
            machine.process_key_down(Key::Space),
            KeyTransition::SpotlightShortcutPressed
        );
    }
    #[test]
    fn tracks_sticky_right_alt_tab() {
        let mut machine = KeyStateMachine::default();
        machine.configure(Key::RightCtrl, Key::LeftAlt, true);
        machine.process_key_down(Key::RightAlt);
        machine.process_key_down(Key::Tab);
        assert_eq!(
            machine.process_key_down(Key::Return),
            KeyTransition::AltTabSwitched { nav_count: 1 }
        );
    }
}
