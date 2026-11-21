use std::cell::RefCell;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

use omnispot_spotlight_native::desktop;
use slint::{ComponentHandle, Timer, TimerMode};

use crate::TaskbarPreview;

pub const TEST_WORKER_ARG: &str = "--taskbar-visibility-test-worker";

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let window = TaskbarPreview::new()?;
    crate::brand_window(window.window());
    window.set_test_seconds(desktop::TASKBAR_TEST_SECONDS as i32);

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
        match start_worker() {
            Ok(child) => {
                *running = Some(child);
                window.set_test_active(true);
                window.set_test_status("Taskbar hide test running.".into());
            }
            Err(error) => window.set_test_status(format!("Could not start the test. {error}").into()),
        }
    });

    let weak = window.as_weak();
    window.on_close_preview(move || {
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
            Ok(Some(status)) if status.success() => "Windows taskbar restored.".into(),
            Ok(Some(_)) => {
                let mut message = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = stderr.read_to_string(&mut message);
                }
                format!("Taskbar test failed. {}", message.trim())
            }
            Err(error) => format!("Could not check the taskbar test. {error}"),
        };
        *worker = None;
        if let Some(window) = weak.upgrade() {
            window.set_test_active(false);
            window.set_test_status(result.into());
        }
    });
    window.run()?;
    Ok(())
}

fn start_worker() -> std::io::Result<Child> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new(std::env::current_exe()?)
        .arg(TEST_WORKER_ARG)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}
