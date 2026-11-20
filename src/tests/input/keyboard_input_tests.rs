use super::*;

#[test]
fn wpf_keys_are_translated_before_sending_input() {
    for (key, vk) in [
        (A, 0x41),
        (Z, 0x5a),
        (D0, 0x30),
        (D9, 0x39),
        (LEFT_ALT, 0xa4),
        (RIGHT_ALT, 0xa5),
        (RIGHT_CTRL, 0xa3),
        (LWIN, 0x5b),
    ] {
        assert_eq!(virtual_key_from_key(key), vk);
    }
    assert_eq!(virtual_key_from_key(SYSTEM), 0);
    assert_eq!(virtual_key_from_key(u32::MAX), 0);
}

#[test]
fn combination_has_atomic_down_down_up_up_order_and_extended_flags() {
    let inputs = combination_inputs(RIGHT_CTRL, A);
    let actual: Vec<_> = inputs
        .iter()
        .map(|input| unsafe {
            assert_eq!(input.r#type, INPUT_KEYBOARD);
            let key = input.Anonymous.ki;
            assert_eq!((key.wScan, key.time, key.dwExtraInfo), (0, 0, 0));
            (key.wVk.0, key.dwFlags.0)
        })
        .collect();
    assert_eq!(
        actual,
        [
            (0xa3, KEYEVENTF_EXTENDEDKEY.0),
            (0x41, 0),
            (0x41, KEYEVENTF_KEYUP.0),
            (0xa3, (KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP).0)
        ]
    );
}

#[test]
fn only_the_original_three_keys_receive_extended_flags() {
    for key in 0..173 {
        let flags = unsafe { key_event_input(key, false).Anonymous.ki.dwFlags };
        assert_eq!(
            flags == KEYEVENTF_EXTENDEDKEY,
            matches!(key, RIGHT_ALT | RIGHT_CTRL | APPS)
        );
    }
}

#[test]
fn physical_key_checks_preserve_modifier_family_fallbacks() {
    for (key, held) in [
        (LEFT_ALT, VK_MENU.0),
        (RIGHT_CTRL, VK_CONTROL.0),
        (LEFT_SHIFT, VK_SHIFT.0),
        (LWIN, VK_RWIN.0),
        (SYSTEM, VK_MENU.0),
        (A, 0x41),
    ] {
        assert!(physically_down(key, |vk| vk == held));
        assert!(!physically_down(key, |_| false));
    }
    assert!(!physically_down(NONE, |_| true));
    assert!(!physically_down(A, |vk| vk == VK_MENU.0));
}
