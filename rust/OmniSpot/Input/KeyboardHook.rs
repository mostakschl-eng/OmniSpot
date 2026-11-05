use crate::configuration::Key;
use std::{
    ffi::c_void,
    sync::{Arc, OnceLock},
};

pub type KeyHandler = Arc<dyn Fn(Key, bool, bool) -> bool + Send + Sync + 'static>;
static HANDLER: OnceLock<KeyHandler> = OnceLock::new();

type Hook = *mut c_void;
#[repr(C)]
struct KeyboardData {
    vk_code: u32,
    scan_code: u32,
    flags: u32,
    time: u32,
    extra_info: usize,
}
#[repr(C)]
struct Message {
    hwnd: *mut c_void,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    point_x: i32,
    point_y: i32,
}

unsafe extern "system" {
    fn SetWindowsHookExW(
        id: i32,
        callback: Option<unsafe extern "system" fn(i32, usize, isize) -> isize>,
        module: *mut c_void,
        thread_id: u32,
    ) -> Hook;
    fn UnhookWindowsHookEx(hook: Hook) -> i32;
    fn CallNextHookEx(hook: Hook, code: i32, w_param: usize, l_param: isize) -> isize;
    fn GetMessageW(message: *mut Message, hwnd: *mut c_void, min: u32, max: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
    fn PostThreadMessageW(thread_id: u32, message: u32, w_param: usize, l_param: isize) -> i32;
    fn GetCurrentThreadId() -> u32;
}

const WH_KEYBOARD_LL: i32 = 13;
const WM_KEYDOWN: usize = 0x0100;
const WM_KEYUP: usize = 0x0101;
const WM_SYSKEYDOWN: usize = 0x0104;
const WM_SYSKEYUP: usize = 0x0105;
const WM_QUIT: u32 = 0x0012;
const LLKHF_INJECTED: u32 = 0x10;

pub struct KeyboardHook {
    hook: Hook,
    thread_id: u32,
}
unsafe impl Send for KeyboardHook {}

impl KeyboardHook {
    pub fn current_thread_id() -> u32 {
        unsafe { GetCurrentThreadId() }
    }
    pub fn start(handler: KeyHandler) -> Result<Self, String> {
        HANDLER
            .set(handler)
            .map_err(|_| "Only one keyboard hook can be active".to_string())?;
        let hook =
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(callback), std::ptr::null_mut(), 0) };
        if hook.is_null() {
            return Err("SetWindowsHookExW failed".to_string());
        }
        Ok(Self {
            hook,
            thread_id: unsafe { GetCurrentThreadId() },
        })
    }
    pub fn pump(&self) -> Result<(), String> {
        unsafe {
            let mut message = std::mem::zeroed::<Message>();
            loop {
                let result = GetMessageW(&mut message, std::ptr::null_mut(), 0, 0);
                if result == -1 {
                    return Err("GetMessageW failed".to_string());
                }
                if result == 0 {
                    return Ok(());
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    pub fn thread_id(&self) -> u32 {
        self.thread_id
    }
    pub fn stop(&self) {
        Self::stop_thread(self.thread_id);
    }
    pub fn stop_thread(thread_id: u32) {
        unsafe {
            PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
        }
    }
}
impl Drop for KeyboardHook {
    fn drop(&mut self) {
        self.stop();
    }
}

unsafe extern "system" fn callback(code: i32, w_param: usize, l_param: isize) -> isize {
    if code >= 0 {
        let data = unsafe { &*(l_param as *const KeyboardData) };
        if let Some(key) = key_from_virtual_key(data.vk_code as u16) {
            let down = matches!(w_param, WM_KEYDOWN | WM_SYSKEYDOWN);
            let up = matches!(w_param, WM_KEYUP | WM_SYSKEYUP);
            if (down || up)
                && HANDLER
                    .get()
                    .is_some_and(|handler| handler(key, down, data.flags & LLKHF_INJECTED != 0))
            {
                return 1;
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param) }
}

fn key_from_virtual_key(vk: u16) -> Option<Key> {
    Some(match vk {
        0x41 => Key::A,
        0x42 => Key::B,
        0x43 => Key::C,
        0x44 => Key::D,
        0x45 => Key::E,
        0x46 => Key::F,
        0x47 => Key::G,
        0x48 => Key::H,
        0x49 => Key::I,
        0x4A => Key::J,
        0x4B => Key::K,
        0x4C => Key::L,
        0x4D => Key::M,
        0x4E => Key::N,
        0x4F => Key::O,
        0x50 => Key::P,
        0x51 => Key::Q,
        0x52 => Key::R,
        0x53 => Key::S,
        0x54 => Key::T,
        0x55 => Key::U,
        0x56 => Key::V,
        0x57 => Key::W,
        0x58 => Key::X,
        0x59 => Key::Y,
        0x5A => Key::Z,
        0x30 => Key::D0,
        0x31 => Key::D1,
        0x32 => Key::D2,
        0x33 => Key::D3,
        0x34 => Key::D4,
        0x35 => Key::D5,
        0x36 => Key::D6,
        0x37 => Key::D7,
        0x38 => Key::D8,
        0x39 => Key::D9,
        0xA2 => Key::LeftCtrl,
        0xA3 => Key::RightCtrl,
        0xA4 => Key::LeftAlt,
        0xA5 => Key::RightAlt,
        0xA0 => Key::LeftShift,
        0xA1 => Key::RightShift,
        0x5B => Key::LWin,
        0x5C => Key::RWin,
        0x5D => Key::Apps,
        0x14 => Key::Capital,
        0x09 => Key::Tab,
        0x25 => Key::Left,
        0x27 => Key::Right,
        0x26 => Key::Up,
        0x28 => Key::Down,
        0x0D => Key::Return,
        0x1B => Key::Escape,
        0x20 => Key::Space,
        _ => return None,
    })
}
