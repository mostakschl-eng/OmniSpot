use std::cell::RefCell;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

use omnispot_spotlight_native::desktop;
use slint::{ComponentHandle, Timer, TimerMode};

use crate::{taskbar_actions, taskbar_dock::Dock, SettingsWindow, TaskbarPreview};

pub const TEST_WORKER_ARG: &str = "--taskbar-visibility-test-worker";

pub struct Taskbar {
    pub window: TaskbarPreview,
    _test_timer: Timer,
    _refresh_timer: Timer,
    dock: Rc<RefCell<Dock>>,
}

impl Taskbar {
    pub fn opened(&self, previous_foreground: isize) {
        refresh_clock(&self.window);
        self.dock.borrow_mut().opened(previous_foreground);
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let taskbar = create_taskbar(None)?;
    let foreground = desktop::foreground_window_handle();
    taskbar.window.show()?;
    if let Some(handle) = crate::host::hwnd(taskbar.window.window()) {
        desktop::raise_taskbar_window(handle);
    }
    taskbar.opened(foreground);
    slint::run_event_loop()?;
    Ok(())
}

pub fn create_taskbar(
    settings: Option<slint::Weak<SettingsWindow>>,
) -> Result<Taskbar, slint::PlatformError> {
    let window = TaskbarPreview::new()?;
    crate::brand_window(window.window());
    let dock = Rc::new(RefCell::new(Dock::new()));
    let action_dock = dock.clone();
    let sender = taskbar_actions::start_worker(4).ok();
    let action_window = window.as_weak();
    window.global::<crate::Actions>().on_run(move |target| {
        let Some(window) = action_window.upgrade() else { return false };
        if target == "OmniSpot Settings" {
            if let Some(settings) = settings.as_ref().and_then(slint::Weak::upgrade) {
                if settings.show().is_err() {
                    window.set_action_status("Could not open OmniSpot Settings.".into());
                    return false;
                }
                window.set_above_other_windows(false);
                if let Some(handle) = crate::host::hwnd(settings.window()) {
                    desktop::focus_window(handle);
                }
                return true;
            }
        }
        let resolved = action_dock.borrow().action_for(target.as_str())
            .or_else(|| taskbar_actions::resolve(target.as_str()).map(|action| (target.to_string(), action)));
        let Some((label, action)) = resolved else { return false };
        let action_result = action_window.clone();
        let accepted = sender.as_ref().is_some_and(|sender| sender.try_send(Box::new(move || {
            let opens_window = action.opens_window();
            let succeeded = action.run();
            let _ = action_result.upgrade_in_event_loop(move |window| {
                if succeeded {
                    if opens_window {
                        window.set_above_other_windows(false);
                    }
                    window.set_action_status("".into());
                } else {
                    window.set_action_status(format!("Could not run {label}.").into());
                }
            });
        })).is_ok());
        window.set_action_status(if accepted { "".into() } else { "Taskbar actions are busy. Try again shortly.".into() });
        accepted
    });
    window.set_test_seconds(desktop::TASKBAR_TEST_SECONDS as i32);

    let weak = window.as_weak();
    window.on_request_drag(move || {
        let _ = weak.upgrade_in_event_loop(|window| {
            crate::drag_window(window.window());
        });
    });

    let worker = Rc::new(RefCell::new(None::<Child>));
    let weak = window.as_weak();
    let running = worker.clone();
    window.on_toggle_test(move |enable| {
        let Some(window) = weak.upgrade() else { return };
        let mut running = running.borrow_mut();
        if !enable {
            if let Some(child) = running.as_mut() {
                child.stdin.take();
            }
            window.set_test_active(false);
            return;
        }
        if running.is_some() {
            return;
        }
        match start_worker(false) {
            Ok(child) => {
                *running = Some(child);
                window.set_test_active(true);
                window.set_test_status("Taskbar hide test running.".into());
            }
            Err(error) => window.set_test_status(format!("Could not start the test. {error}").into()),
        }
    });

    let weak = window.as_weak();
    let running = worker.clone();
    window.on_close_preview(move || {
        if let Some(child) = running.borrow_mut().as_mut() {
            child.stdin.take();
        }
        if let Some(window) = weak.upgrade() {
            let _ = window.hide();
        }
    });

    let timer = Timer::default();
    let weak = window.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(150), move || {
        let mut worker = worker.borrow_mut();
        let Some(child) = worker.as_mut() else { return };
        let result = match child.try_wait() {
            Ok(None) => return,
            Ok(Some(status)) => match worker_result(child, status) {
                Ok(()) => "Windows taskbar restored.".into(),
                Err(error) => format!("Taskbar test failed. {error}"),
            },
            Err(error) => format!("Could not check the taskbar test. {error}"),
        };
        *worker = None;
        if let Some(window) = weak.upgrade() {
            window.set_test_active(false);
            window.set_test_status(result.into());
        }
    });

    let current_dock = dock.clone();
    let weak = window.as_weak();
    let mut next_clock = Instant::now();
    let refresh_timer = Timer::default();
    refresh_timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
        let Some(window) = weak.upgrade() else { return };
        if !window.window().is_visible() {
            return;
        }
        let now = Instant::now();
        if now >= next_clock {
            refresh_clock(&window);
            next_clock = now + Duration::from_secs(1);
        }
        current_dock.borrow_mut().refresh(&window, now);
    });
    Ok(Taskbar { window, _test_timer: timer, _refresh_timer: refresh_timer, dock })
}

fn refresh_clock(window: &TaskbarPreview) {
    let clock = desktop::taskbar_clock();
    window.set_clock_time(clock.time.into());
    window.set_clock_date(clock.date.into());
    window.set_clock_short_date(clock.short_date.into());
    window.set_clock_month(crate::CalendarMonth {
        label: clock.month.into(),
        first_weekday: clock.first_weekday,
        days: clock.days,
        today: clock.today,
    });
}

pub fn start_worker(persistent: bool) -> std::io::Result<Child> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new(std::env::current_exe()?);
    command.arg(TEST_WORKER_ARG);
    if persistent {
        command.arg("--persistent");
    }
    command
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

pub fn worker_result(child: &mut Child, status: ExitStatus) -> Result<(), String> {
    if status.success() {
        return Ok(());
    }
    let mut message = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut message);
    }
    if message.trim().is_empty() {
        return Err(format!("The taskbar visibility worker exited with {status}."));
    }
    Err(message.trim().to_owned())
}
