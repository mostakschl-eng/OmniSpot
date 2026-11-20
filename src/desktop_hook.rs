use crate::configuration::configuration::{ApplicationConfiguration, Configuration};
use crate::input::{
    dynamic_mode_service::DynamicModeService,
    hook::{Hook, HookHost, KeyboardHookEvent, Warning},
    keyboard_input,
    peeker::Peeker,
};
use crate::runtime::RuntimeError;
use crate::stats::{i_app_registry_cache::IAppRegistryCache, stats_event::StatsEvent};
use crate::switcher::Switcher;
use crate::window_discovery::{
    app_name_resolver::AppNameResolver, process_inspector, window_enumerator::WindowEnumerator,
};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;
use windows::core::w;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::System::{LibraryLoader::GetModuleHandleW, Threading::CreateMutexW};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

pub enum CaptureEvent {
    StartOverlayTimer(Duration),
    CancelOverlayTimer,
    ShowOverlay(Vec<ApplicationConfiguration>, bool),
    HideOverlay,
    Warn { restricted: bool },
    ToggleSpotlight,
    PotentialElevation,
    Error(String),
}

struct DisplayNames;
impl IAppRegistryCache for DisplayNames {
    fn get_display_name(&self, process_name: &str, process_path: Option<&str>) -> String {
        crate::ffi::desktop_display_name(process_name, process_path)
    }
}

struct NativeHost {
    events: Sender<CaptureEvent>,
    overlay_delay: Duration,
    overlay_visible: bool,
}

impl NativeHost {
    fn emit(&self, event: CaptureEvent) {
        let _ = self.events.send(event);
    }
}

impl HookHost for NativeHost {
    fn subscribe_keyboard(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn unsubscribe_keyboard(&mut self) {}
    fn dispose_keyboard(&mut self) {}
    fn configure_overlay_timer(&mut self, timeout_ms: i32) {
        self.overlay_delay = Duration::from_millis(timeout_ms.max(0) as u64);
    }
    fn start_overlay_timer(&mut self) {
        self.emit(CaptureEvent::StartOverlayTimer(self.overlay_delay));
    }
    fn cancel_overlay_timer(&mut self) {
        self.emit(CaptureEvent::CancelOverlayTimer);
    }
    fn overlay_is_visible(&self) -> bool {
        self.overlay_visible
    }
    fn show_overlay(&mut self, applications: &[ApplicationConfiguration], dynamic: bool) {
        self.emit(CaptureEvent::ShowOverlay(applications.to_vec(), dynamic));
    }
    fn hide_overlay(&mut self) {
        self.emit(CaptureEvent::HideOverlay);
    }
    fn show_warning(&mut self, warning: Warning) {
        self.emit(CaptureEvent::Warn {
            restricted: warning == Warning::KeyEventsStealing,
        });
    }
    fn toggle_spotlight(&mut self) {
        self.emit(CaptureEvent::ToggleSpotlight);
    }
    fn enqueue_stats(&mut self, event: StatsEvent) {
        crate::ffi::enqueue_desktop_stats(event);
    }
    fn monitor_potential_elevation(&mut self, process_path: &str) {
        let events = self.events.clone();
        let process_path = process_path.to_owned();
        let _ = std::thread::Builder::new().spawn(move || {
            if process_inspector::wait_for_potential_elevation(&process_path, 2000) == Ok(true) {
                let _ = events.send(CaptureEvent::PotentialElevation);
            }
        });
    }
    fn log_error(&mut self, error: &RuntimeError) {
        self.emit(CaptureEvent::Error(error.message.clone()));
    }
}

type Engine = Hook<WindowEnumerator, AppNameResolver<DisplayNames>, NativeHost>;

thread_local! {
    static ENGINE: RefCell<Option<(Engine, String)>> = const { RefCell::new(None) };
}

pub struct DesktopInstance(HANDLE);

impl DesktopInstance {
    pub fn acquire() -> Result<Self, String> {
        let instance = unsafe { CreateMutexW(None, false, w!("OmniSpotMutex")) }
            .map_err(|error| error.to_string())?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            let _ = unsafe { CloseHandle(instance) };
            return Err("OmniSpot is already running. Exit the existing instance before starting the Rust host.".into());
        }

        Ok(Self(instance))
    }
}

impl Drop for DesktopInstance {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub struct DesktopCapture {
    hook: HHOOK,
    _thread: PhantomData<Rc<()>>,
}

impl DesktopCapture {
    pub fn install(config: Configuration) -> Result<(Self, Receiver<CaptureEvent>), String> {
        let (engine, receiver) = create_engine(config)?;
        let mut locale = [0; 85];
        let length = unsafe { GetUserDefaultLocaleName(&mut locale) };
        let culture = String::from_utf16_lossy(&locale[..length.saturating_sub(1).max(0) as usize]);
        ENGINE.with(|slot| *slot.borrow_mut() = Some((engine, culture)));
        let handle = unsafe {
            GetModuleHandleW(None).and_then(|module| {
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_callback), module, 0)
            })
        };
        match handle {
            Ok(hook) => Ok((
                Self {
                    hook,
                    _thread: PhantomData,
                },
                receiver,
            )),
            Err(error) => {
                ENGINE.with(|slot| {
                    slot.borrow_mut().take();
                });
                Err(error.to_string())
            }
        }
    }

    pub fn update(config: Configuration) {
        with_engine(|engine, _| engine.update_configuration(config));
    }

    pub fn preview_overlay(enabled: bool) {
        with_engine(|engine, _| engine.set_overlay_preview_enabled(enabled));
    }

    pub fn set_overlay_visible(&self, visible: bool) {
        with_engine(|engine, _| engine.host.overlay_visible = visible);
    }

    pub fn show_overlay_if_enabled(&self) {
        with_engine(|engine, _| engine.show_overlay_if_enabled());
    }

    pub fn potential_elevation_detected(&self) {
        with_engine(|engine, _| engine.potential_elevation_detected());
    }
}

fn create_engine(config: Configuration) -> Result<(Engine, Receiver<CaptureEvent>), String> {
    let (events, receiver) = mpsc::channel();
    let mut engine = Hook::new(
        Switcher::new(WindowEnumerator::default()),
        Peeker::default(),
        WindowEnumerator::default(),
        DynamicModeService {
            window_enumerator: WindowEnumerator::default(),
            app_name_resolver: AppNameResolver {
                app_registry_cache: DisplayNames,
            },
            packaged_apps_service: Default::default(),
        },
        NativeHost {
            events,
            overlay_delay: Duration::ZERO,
            overlay_visible: false,
        },
    );
    engine.start(config).map_err(|error| error.message)?;
    Ok((engine, receiver))
}

impl Drop for DesktopCapture {
    fn drop(&mut self) {
        let _ = unsafe { UnhookWindowsHookEx(self.hook) };
        ENGINE.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

fn with_engine<T>(action: impl FnOnce(&mut Engine, &str) -> T) -> Option<T> {
    ENGINE.with(|slot| {
        let mut storage = slot.try_borrow_mut().ok()?;
        let (engine, culture) = storage.as_mut()?;
        Some(action(engine, culture))
    })
}

unsafe extern "system" fn keyboard_callback(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == 0 && lparam.0 != 0 {
        let message = wparam.0 as u32;
        if matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP) {
            let input = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            if input.flags.0 & 0x10 == 0 {
                // Windows callbacks must always return even if an event handler fails.
                let suppressed = std::panic::catch_unwind(|| {
                    dispatch_keyboard(message, input.vkCode, input.flags.0)
                })
                .unwrap_or(false);
                if suppressed {
                    return LRESULT(1);
                }
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn dispatch_keyboard(message: u32, vk: u32, flags: u32) -> bool {
    let mut event = KeyboardHookEvent {
        key: keyboard_input::key_from_virtual_key(vk),
        is_key_down: matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN),
        flags,
        suppress_key_press: false,
    };
    with_engine(|engine, culture| engine.keyboard_pressed(&mut event, culture));
    event.suppress_key_press
}

#[cfg(test)]
#[test]
fn native_modifier_events_reach_the_overlay_timer_and_window_queue() {
    let modifier = keyboard_input::LEFT_SHIFT;
    let vk = u32::from(keyboard_input::virtual_key_from_key(modifier));
    let (engine, events) = create_engine(Configuration {
        modifier,
        overlay_enabled: true,
        overlay_show_delay_ms: 1000,
        ..Configuration::default()
    })
    .unwrap();
    ENGINE.with(|slot| *slot.borrow_mut() = Some((engine, "en-US".into())));
    assert!(!dispatch_keyboard(WM_KEYDOWN, vk, 0));
    assert!(
        matches!(events.try_recv().unwrap(), CaptureEvent::StartOverlayTimer(delay) if delay == Duration::from_secs(1))
    );
    with_engine(|engine, _| engine.show_overlay_if_enabled());
    assert!(matches!(
        events.try_recv().unwrap(),
        CaptureEvent::ShowOverlay(..)
    ));
    assert!(!dispatch_keyboard(WM_KEYUP, vk, 0));
    assert!(matches!(
        events.try_recv().unwrap(),
        CaptureEvent::CancelOverlayTimer
    ));
    assert!(matches!(
        events.try_recv().unwrap(),
        CaptureEvent::HideOverlay
    ));
    ENGINE.with(|slot| {
        slot.borrow_mut().take();
    });
}
