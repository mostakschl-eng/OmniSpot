use super::*;
use KeyTransition::*;

fn machine(modifier: u32) -> KeyStateMachine {
    let mut machine = KeyStateMachine::default();
    machine.configure(modifier, LEFT_ALT, true);
    machine
}

#[test]
fn modifier_repeat_and_clean_release_preserve_side_effects() {
    for (key, side_effect) in [
        (LWIN, true),
        (RWIN, true),
        (LEFT_ALT, true),
        (RIGHT_ALT, true),
        (APPS, true),
        (CAPITAL, true),
        (LEFT_CTRL, false),
        (LEFT_SHIFT, false),
    ] {
        let mut state = machine(key);
        assert_eq!(
            state.process_key_down(key),
            ModifierPressed {
                is_first_press: true,
                has_side_effect: side_effect
            }
        );
        let tick = state.modifier_pressed_at_tick;
        assert_eq!(
            state.process_key_down(key),
            ModifierPressed {
                is_first_press: false,
                has_side_effect: side_effect
            }
        );
        assert_eq!(state.modifier_pressed_at_tick, tick);
        assert!(
            matches!(state.process_key_up(key), ModifierReleasedClean { has_side_effect, held_duration_ms }
            if has_side_effect == side_effect && held_duration_ms >= 0)
        );
        assert!(!state.is_modifier_held());
        assert_eq!(state.process_key_up(key), NoOp);
    }
}

#[test]
fn letters_and_digits_leave_action_state_after_either_release_order() {
    for key in (A..=Z).chain(D0..=D9) {
        for release_action_first in [true, false] {
            let mut state = machine(LWIN);
            assert_eq!(state.process_key_down(key), NoOp);
            state.process_key_down(LWIN);
            assert_eq!(
                state.process_key_down(key),
                if key >= A {
                    LetterKeyPressed(key)
                } else {
                    DigitKeyPressed(key)
                }
            );
            if release_action_first {
                assert_eq!(state.process_key_up(key), NoOp);
            }
            assert_eq!(
                state.process_key_up(LWIN),
                ModifierReleasedAfterAction {
                    has_side_effect: true
                }
            );
        }
    }
}

#[test]
fn unexpected_modifier_blocks_actions_until_released() {
    let mut state = machine(LWIN);
    state.process_key_down(LEFT_CTRL);
    state.process_key_down(LWIN);
    assert_eq!(state.process_key_down(A), NoOp);
    assert_eq!(state.process_key_down(D0), NoOp);
    state.process_key_up(LEFT_CTRL);
    assert_eq!(state.process_key_down(A), LetterKeyPressed(A));
}

#[test]
fn unrelated_key_reset_and_reconfiguration_clear_held_state() {
    let mut state = machine(LWIN);
    state.process_key_down(LWIN);
    assert_eq!(state.process_key_down(ESCAPE), UnrelatedKeyReset);
    assert_eq!(state.process_key_up(LWIN), NoOp);
    state.process_key_down(LWIN);
    state.configure(LEFT_CTRL, LEFT_ALT, true);
    assert!(!state.is_modifier_held());
    assert!(!state.configured_modifier_has_side_effect());
    assert_eq!(state.process_key_down(LWIN), NoOp);
    assert_eq!(
        state.process_key_down(LEFT_CTRL),
        ModifierPressed {
            is_first_press: true,
            has_side_effect: false
        }
    );
}

#[test]
fn left_alt_navigation_counts_tabs_and_arrows_and_resets_sessions() {
    let mut state = machine(LWIN);
    state.process_key_down(LEFT_ALT);
    assert_eq!(state.process_key_down(LEFT), NoOp);
    for key in [TAB, TAB, LEFT, RIGHT, UP, DOWN] {
        assert_eq!(state.process_key_down(key), NoOp);
    }
    assert_eq!(state.process_key_up(LEFT_ALT), AltTabSwitched(6));
    state.process_key_down(LEFT_ALT);
    assert_eq!(state.process_key_up(LEFT_ALT), NoOp);
}

#[test]
fn right_alt_navigation_stays_sticky_until_enter_or_escape() {
    let mut state = machine(LWIN);
    state.process_key_down(RIGHT_ALT);
    state.process_key_down(TAB);
    assert_eq!(state.process_key_up(RIGHT_ALT), NoOp);
    state.process_key_down(RIGHT);
    assert_eq!(state.process_key_down(RETURN), AltTabSwitched(2));
    assert_eq!(state.process_key_down(RETURN), NoOp);
    state.process_key_down(RIGHT_ALT);
    state.process_key_down(TAB);
    state.process_key_down(ESCAPE);
    assert_eq!(state.process_key_down(RETURN), NoOp);
}

#[test]
fn configured_alt_is_not_counted_as_alt_tab_navigation() {
    let mut state = machine(LEFT_ALT);
    state.process_key_down(LEFT_ALT);
    assert_eq!(state.process_key_down(TAB), UnrelatedKeyReset);
    assert_eq!(state.process_key_up(LEFT_ALT), NoOp);
}

#[test]
fn spotlight_accepts_either_side_of_the_configured_modifier_family() {
    for (configured, other) in [
        (LEFT_ALT, RIGHT_ALT),
        (LEFT_CTRL, RIGHT_CTRL),
        (LEFT_SHIFT, RIGHT_SHIFT),
        (LWIN, RWIN),
    ] {
        let mut state = machine(CAPITAL);
        state.configure(CAPITAL, configured, true);
        state.process_key_down(other);
        assert_eq!(state.process_key_down(SPACE), SpotlightShortcutPressed);
        state.reset();
        assert_eq!(state.process_key_down(SPACE), NoOp);
        state.configure(CAPITAL, configured, false);
        state.process_key_down(other);
        assert_eq!(state.process_key_down(SPACE), NoOp);
    }
}

#[test]
fn reset_clears_alt_tab_and_unexpected_modifier_tracking() {
    let mut state = machine(LWIN);
    state.process_key_down(RIGHT_ALT);
    state.process_key_down(TAB);
    state.reset();
    assert_eq!(state.process_key_down(RETURN), NoOp);
    state.process_key_down(LWIN);
    assert_eq!(state.process_key_down(A), LetterKeyPressed(A));
}
