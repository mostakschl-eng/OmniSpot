use omnispot_spotlight_native::desktop::{self, DockWindow};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::mpsc::{sync_channel, SyncSender};

pub type Task = Box<dyn FnOnce() + Send>;

pub fn start_worker(capacity: usize) -> std::io::Result<SyncSender<Task>> {
    let (sender, receiver) = sync_channel::<Task>(capacity);
    std::thread::Builder::new()
        .name("omnispot-taskbar-actions".into())
        .spawn(move || {
            for task in receiver {
                task();
            }
        })?;
    Ok(sender)
}

pub enum Action {
    Activate(DockWindow),
    Open(String),
    Media(String),
    Process(&'static str, Vec<String>),
}

impl Action {
    pub fn opens_window(&self) -> bool {
        !matches!(self, Self::Media(_))
    }

    pub fn run(self) -> bool {
        const NO_WINDOW: u32 = 0x0800_0000;
        match self {
            Self::Activate(window) => desktop::activate_dock_window(&window),
            Self::Open(target) => desktop::open_in_shell(&target).is_ok(),
            Self::Media(action) => desktop::send_media_key(&action),
            Self::Process(program, args) => Command::new(program)
                .args(args)
                .creation_flags(NO_WINDOW)
                .spawn()
                .is_ok(),
        }
    }
}

pub fn resolve(target: &str) -> Option<Action> {
    fn process(program: &'static str, args: &[&str]) -> Action {
        Action::Process(program, args.iter().map(|arg| (*arg).to_owned()).collect())
    }
    let start = |what: &str| process("cmd", &["/C", "start", "", what]);
    Some(match target {
        // power
        "Lock" => process("rundll32.exe", &["user32.dll,LockWorkStation"]),
        "Sleep" => process("rundll32.exe", &["powrprof.dll,SetSuspendState", "0,1,0"]),
        "Restart" => process("shutdown", &["/r", "/t", "0"]),
        "Shut down" => process("shutdown", &["/s", "/t", "0"]),
        "Sign out" => process("shutdown", &["/l"]),
        // desktop / shell
        "Show desktop" => process(
            "powershell",
            &[
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                "(New-Object -ComObject Shell.Application).ToggleDesktop()",
            ],
        ),
        "Run" => process(
            "explorer.exe",
            &["shell:::{2559a1f3-21d7-11d4-bdaf-00c04f60b9f0}"],
        ),
        "Terminal (Admin)" => process(
            "powershell",
            &[
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                "Start-Process wt -Verb RunAs",
            ],
        ),
        "powercfg.cpl" | "eventvwr.msc" | "devmgmt.msc" | "ncpa.cpl" | "diskmgmt.msc"
        | "compmgmt.msc" | "wt" | "taskmgr" | "explorer" => start(target),
        // media keys
        "media:prev" | "media:next" | "media:play" | "media:mute" => Action::Media(target.into()),
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
        "Network & internet" => Action::Open("ms-settings:network".into()),
        "Sound" => Action::Open("ms-settings:sound".into()),
        "Display" => Action::Open("ms-settings:display".into()),
        other if other.starts_with("ms-settings:") => Action::Open(other.into()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::start_worker;
    use std::sync::mpsc::{channel, TrySendError};
    use std::time::Duration;

    #[test]
    fn a_blocked_action_does_not_block_more_ui_submissions() {
        let queue = start_worker(1).unwrap();
        let (started, running) = channel();
        let (release, blocked) = channel();
        let (finished, completed) = channel();
        assert!(queue
            .try_send(Box::new(move || {
                started.send(()).unwrap();
                let _ = blocked.recv();
            }))
            .is_ok());
        running.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(queue
            .try_send(Box::new(move || {
                finished.send(()).unwrap();
            }))
            .is_ok());
        assert!(matches!(
            queue.try_send(Box::new(|| {})),
            Err(TrySendError::Full(_))
        ));
        release.send(()).unwrap();
        completed.recv_timeout(Duration::from_secs(2)).unwrap();
    }
}
