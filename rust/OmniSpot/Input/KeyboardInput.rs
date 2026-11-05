use crate::configuration::Key;

#[repr(C)]
#[derive(Clone, Copy)]
struct KeyboardInput {
    virtual_key: u16,
    scan_code: u16,
    flags: u32,
    time: u32,
    extra_info: usize,
}
#[repr(C)]
#[derive(Clone, Copy)]
union InputData {
    keyboard: KeyboardInput,
    _padding: [u8; 32],
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Input {
    input_type: u32,
    data: InputData,
}

const INPUT_KEYBOARD: u32 = 1;
const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
const KEYEVENTF_KEYUP: u32 = 0x0002;

unsafe extern "system" {
    fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
}

pub fn send_synthetic_key_down_up(key: Key) -> bool {
    send(&[key_event(key, false), key_event(key, true)])
}

pub fn send_synthetic_combination(modifier: Key, action: Key) -> bool {
    send(&[
        key_event(modifier, false),
        key_event(action, false),
        key_event(action, true),
        key_event(modifier, true),
    ])
}

fn send(inputs: &[Input]) -> bool {
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<Input>() as i32,
        ) == inputs.len() as u32
    }
}

fn key_event(key: Key, key_up: bool) -> Input {
    let virtual_key = virtual_key(key);
    let extended = matches!(
        key,
        Key::RightCtrl | Key::RightAlt | Key::Apps | Key::LWin | Key::RWin
    );
    Input {
        input_type: INPUT_KEYBOARD,
        data: InputData {
            keyboard: KeyboardInput {
                virtual_key,
                scan_code: 0,
                flags: (if extended { KEYEVENTF_EXTENDEDKEY } else { 0 })
                    | (if key_up { KEYEVENTF_KEYUP } else { 0 }),
                time: 0,
                extra_info: 0,
            },
        },
    }
}

fn virtual_key(key: Key) -> u16 {
    if let Some(index) = key.letter_index() {
        return b'A' as u16 + index;
    }
    match key {
        Key::D0 => 0x30,
        Key::D1 => 0x31,
        Key::D2 => 0x32,
        Key::D3 => 0x33,
        Key::D4 => 0x34,
        Key::D5 => 0x35,
        Key::D6 => 0x36,
        Key::D7 => 0x37,
        Key::D8 => 0x38,
        Key::D9 => 0x39,
        Key::LeftCtrl => 0xA2,
        Key::RightCtrl => 0xA3,
        Key::LeftAlt => 0xA4,
        Key::RightAlt => 0xA5,
        Key::LeftShift => 0xA0,
        Key::RightShift => 0xA1,
        Key::LWin => 0x5B,
        Key::RWin => 0x5C,
        Key::Apps => 0x5D,
        Key::Capital => 0x14,
        Key::Tab => 0x09,
        Key::Left => 0x25,
        Key::Right => 0x27,
        Key::Up => 0x26,
        Key::Down => 0x28,
        Key::Return => 0x0D,
        Key::Escape => 0x1B,
        Key::Space => 0x20,
        Key::None => 0,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::virtual_key;
    use crate::configuration::Key;
    #[test]
    fn maps_letters_and_extended_modifiers_to_windows_virtual_keys() {
        assert_eq!(virtual_key(Key::A), 0x41);
        assert_eq!(virtual_key(Key::Z), 0x5A);
        assert_eq!(virtual_key(Key::RightCtrl), 0xA3);
        assert_eq!(virtual_key(Key::Space), 0x20);
    }
}
