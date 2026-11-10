use windows::Win32::UI::Input::KeyboardAndMouse::*;

// Keep WPF Key values used in Configuration; they are not Windows virtual-key codes.
pub(crate) const NONE: u32 = 0;
pub(crate) const TAB: u32 = 3;
pub(crate) const RETURN: u32 = 6;
pub(crate) const CAPITAL: u32 = 8;
pub(crate) const ESCAPE: u32 = 13;
pub(crate) const SPACE: u32 = 18;
pub(crate) const LEFT: u32 = 23;
pub(crate) const UP: u32 = 24;
pub(crate) const RIGHT: u32 = 25;
pub(crate) const DOWN: u32 = 26;
pub(crate) const D0: u32 = 34;
pub(crate) const D1: u32 = 35;
pub(crate) const D9: u32 = 43;
pub(crate) const A: u32 = 44;
pub(crate) const Z: u32 = 69;
pub(crate) const LWIN: u32 = 70;
pub(crate) const RWIN: u32 = 71;
pub(crate) const APPS: u32 = 72;
pub(crate) const LEFT_SHIFT: u32 = 116;
pub(crate) const RIGHT_SHIFT: u32 = 117;
pub(crate) const LEFT_CTRL: u32 = 118;
pub(crate) const RIGHT_CTRL: u32 = 119;
pub(crate) const LEFT_ALT: u32 = 120;
pub(crate) const RIGHT_ALT: u32 = 121;
pub(crate) const SYSTEM: u32 = 156;

const VIRTUAL_KEYS: [u16; 173] = [
    0, 3, 8, 9, 0, 12, 13, 19, 20, 21, 23, 24, 25, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38,
    39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 65, 66, 67, 68, 69,
    70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93,
    95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114,
    115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133,
    134, 135, 144, 145, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174,
    175, 176, 177, 178, 179, 180, 181, 182, 183, 186, 187, 188, 189, 190, 191, 192, 193, 194, 219,
    220, 221, 222, 223, 226, 229, 0, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251,
    252, 253, 254, 0,
];

pub(crate) fn virtual_key_from_key(key: u32) -> u16 {
    VIRTUAL_KEYS.get(key as usize).copied().unwrap_or(0)
}

fn key_event_input(key: u32, up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if matches!(key, RIGHT_CTRL | RIGHT_ALT | APPS) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(virtual_key_from_key(key)),
                dwFlags: flags,
                ..Default::default()
            },
        },
    }
}

pub(crate) fn send_synthetic_key_down_up(key: u32) -> bool {
    let inputs = [key_event_input(key, false), key_event_input(key, true)];
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) == inputs.len() as u32 }
}

pub(crate) fn send_synthetic_combination(modifier: u32, action_key: u32) -> bool {
    let inputs = combination_inputs(modifier, action_key);
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) == inputs.len() as u32 }
}

fn combination_inputs(modifier: u32, action_key: u32) -> [INPUT; 4] {
    [
        key_event_input(modifier, false),
        key_event_input(action_key, false),
        key_event_input(action_key, true),
        key_event_input(modifier, true),
    ]
}

pub(crate) fn is_key_physically_down(key: u32) -> bool {
    physically_down(key, |vk| unsafe {
        GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0
    })
}

fn physically_down(key: u32, down: impl Fn(u16) -> bool) -> bool {
    if key == SYSTEM {
        return down(VK_MENU.0);
    }
    let vk = virtual_key_from_key(key);
    if vk == 0 {
        return false;
    }
    if down(vk) {
        return true;
    }
    match key {
        LEFT_ALT | RIGHT_ALT => down(VK_MENU.0),
        LEFT_CTRL | RIGHT_CTRL => down(VK_CONTROL.0),
        LEFT_SHIFT | RIGHT_SHIFT => down(VK_SHIFT.0),
        LWIN | RWIN => down(VK_LWIN.0) || down(VK_RWIN.0),
        _ => false,
    }
}

#[cfg(test)]
#[path = "../tests/Input/KeyboardInputTests.rs"]
mod tests;
