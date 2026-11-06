use async_channel::Sender;
use std::{
    os::windows::ffi::OsStrExt,
    thread::{self, JoinHandle},
};
use windows::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::{
        Shell::{NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW},
        WindowsAndMessaging::{
            AppendMenuW, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CreatePopupMenu, CreateWindowExW,
            DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, GWLP_USERDATA,
            GetCursorPos, GetMessageW, GetWindowLongPtrW, MF_SEPARATOR, MF_STRING, MSG,
            PostThreadMessageW, RegisterClassW, SW_HIDE, SetForegroundWindow, SetWindowLongPtrW,
            ShowWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TrackPopupMenu, TranslateMessage, WM_APP,
            WM_COMMAND, WM_LBUTTONDBLCLK, WM_NCCREATE, WM_QUIT, WM_RBUTTONUP, WNDCLASSW,
            WS_EX_TOOLWINDOW, WS_POPUP,
        },
    },
};
use windows::core::{PCWSTR, w};

const TRAY_ID: u32 = 1;
const OPEN_SETTINGS: usize = 1001;
const EXIT_APPLICATION: usize = 1002;
const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;
const CLASS_NAME: PCWSTR = w!("OmniSpot.TrayWindow");

pub(crate) struct TrayIcon {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl TrayIcon {
    pub(crate) fn spawn(events: Sender<crate::input::hook::InputEvent>) -> Result<Self, String> {
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("omnispot-tray".to_string())
            .spawn(move || tray_thread(events, ready_sender))
            .map_err(|error| format!("Could not start tray thread: {error}"))?;
        let thread_id = ready_receiver
            .recv()
            .map_err(|_| "Tray thread stopped before initialization".to_string())??;
        Ok(Self {
            thread_id,
            thread: Some(thread),
        })
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn tray_thread(
    events: Sender<crate::input::hook::InputEvent>,
    ready: std::sync::mpsc::SyncSender<Result<u32, String>>,
) {
    let thread_id = unsafe { GetCurrentThreadId() };
    let Ok(instance) = (unsafe { GetModuleHandleW(None) }) else {
        let _ = ready.send(Err("Could not get the OmniSpot module handle".into()));
        return;
    };
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(tray_window_proc),
        hInstance: HINSTANCE(instance.0),
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };
    unsafe {
        let _ = RegisterClassW(&class);
    }
    let state_ptr = Box::into_raw(Box::new(TrayState { events }));
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            CLASS_NAME,
            w!("OmniSpot Tray"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            Some(state_ptr.cast()),
        )
    };
    if hwnd.is_err() {
        unsafe {
            drop(Box::from_raw(state_ptr));
        }
        let _ = ready.send(Err("Could not create the OmniSpot tray window".into()));
        return;
    }
    let hwnd = hwnd.unwrap();
    let data = notification_data(hwnd);
    if !unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool() {
        unsafe {
            drop(Box::from_raw(state_ptr));
        }
        let _ = ready.send(Err("Windows rejected the OmniSpot notification icon".into()));
        return;
    }
    let _ = ready.send(Ok(thread_id));
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    let mut message = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 <= 0 {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
        let _ = DestroyWindow(hwnd);
        drop(Box::from_raw(state_ptr));
    }
}

struct TrayState {
    events: Sender<crate::input::hook::InputEvent>,
}

unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        match message {
            TRAY_CALLBACK_MESSAGE => match lparam.0 as u32 {
                WM_LBUTTONDBLCLK => send_event(hwnd, crate::input::hook::InputEvent::OpenSettings),
                WM_RBUTTONUP => show_menu(hwnd),
                _ => {}
            },
            WM_COMMAND => match wparam.0 & 0xffff {
                OPEN_SETTINGS => send_event(hwnd, crate::input::hook::InputEvent::OpenSettings),
                EXIT_APPLICATION => send_event(hwnd, crate::input::hook::InputEvent::Exit),
                _ => {}
            },
            _ => return DefWindowProcW(hwnd, message, wparam, lparam),
        }
        LRESULT(0)
    }
}

unsafe fn send_event(hwnd: HWND, event: crate::input::hook::InputEvent) {
    unsafe {
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const TrayState;
        if !state_ptr.is_null() {
            let _ = (*state_ptr).events.try_send(event);
        }
    }
}

unsafe fn show_menu(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let open = wide("Settings");
        let exit = wide("Exit");
        let _ = AppendMenuW(menu, MF_STRING, OPEN_SETTINGS, PCWSTR(open.as_ptr()));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, EXIT_APPLICATION, PCWSTR(exit.as_ptr()));
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            point.x,
            point.y,
            None,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
    }
}

fn notification_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        uFlags: NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: TRAY_CALLBACK_MESSAGE,
        ..Default::default()
    };
    let tooltip = wide("OmniSpot");
    data.szTip[..tooltip.len()].copy_from_slice(&tooltip);
    data
}

fn wide(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(Some(0))
        .collect()
}
