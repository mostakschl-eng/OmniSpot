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
    window.global::<crate::Actions>().on_run(|target| launch(target.as_str()));
    window.set_test_seconds(desktop::TASKBAR_TEST_SECONDS as i32);

    let weak = window.as_weak();
    window.on_request_drag(move || {
        if let Some(window) = weak.upgrade() {
            crate::drag_window(window.window());
        }
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



/// Runs a real Windows action for a taskbar button. Returns false for unknown targets.
fn launch(target: &str) -> bool {
    const NO_WINDOW: u32 = 0x0800_0000;
    let spawn = |program: &str, args: &[&str]| {
        Command::new(program).args(args).creation_flags(NO_WINDOW).spawn().is_ok()
    };
    let start = |what: &str| spawn("cmd", &["/C", "start", "", what]);
    let key = |code: u32| {
        spawn(
            "powershell",
            &["-NoProfile", "-WindowStyle", "Hidden", "-Command", &format!("(New-Object -ComObject WScript.Shell).SendKeys([char]{code})")],
        )
    };
    match target {
        // power
        "Lock" => spawn("rundll32.exe", &["user32.dll,LockWorkStation"]),
        "Sleep" => spawn("rundll32.exe", &["powrprof.dll,SetSuspendState", "0,1,0"]),
        "Restart" => spawn("shutdown", &["/r", "/t", "0"]),
        "Shut down" => spawn("shutdown", &["/s", "/t", "0"]),
        "Sign out" => spawn("shutdown", &["/l"]),
        // desktop / shell
        "Show desktop" => spawn(
            "powershell",
            &["-NoProfile", "-WindowStyle", "Hidden", "-Command", "(New-Object -ComObject Shell.Application).ToggleDesktop()"],
        ),
        "Run" => spawn("explorer.exe", &["shell:::{2559a1f3-21d7-11d4-bdaf-00c04f60b9f0}"]),
        "Terminal (Admin)" => spawn(
            "powershell",
            &["-NoProfile", "-WindowStyle", "Hidden", "-Command", "Start-Process wt -Verb RunAs"],
        ),
        "powercfg.cpl" | "eventvwr.msc" | "devmgmt.msc" | "ncpa.cpl" | "diskmgmt.msc" | "compmgmt.msc" | "wt"
        | "taskmgr" | "explorer" => start(target),
        // media keys
        "media:prev" => key(177),
        "media:next" => key(176),
        "media:play" => key(179),
        "media:mute" => key(173),
        // dock apps
        "Microsoft Edge" => start("msedge"),
        "File Explorer" => start("explorer"),
        "Word" => start("winword"),
        "Excel" => start("excel"),
        "PowerPoint" => start("powerpnt"),
        "Teams" => start("ms-teams:"),
        "Outlook" => start("outlook"),
        "Visual Studio Code" => start("code"),
        // search result shortcuts
        "Network & internet" => start("ms-settings:network"),
        "Sound" => start("ms-settings:sound"),
        "Display" => start("ms-settings:display"),
        other if other.starts_with("ms-settings:") => start(other),
        _ => false,
    }
}