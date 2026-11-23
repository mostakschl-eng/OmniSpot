use crate::{taskbar_actions::Action, DockApp, DockWindow, TaskbarPreview};
use omnispot_spotlight_native::desktop::{self, DockApp as NativeApp, DockWindow as NativeWindow};
use slint::{Image, ModelRc, SharedPixelBuffer, VecModel};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

pub struct Dock {
    apps: Vec<NativeApp>,
    icons: HashMap<IconKey, Image>,
    pending: Option<Receiver<Snapshot>>,
    next_refresh: Instant,
    last_foreground: isize,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct IconKey {
    path: String,
    index: Option<i32>,
}

impl From<&NativeApp> for IconKey {
    fn from(app: &NativeApp) -> Self {
        Self {
            path: app.icon_path.clone(),
            index: app.icon_index,
        }
    }
}

struct Snapshot {
    apps: Vec<NativeApp>,
    icons: Vec<(IconKey, Option<Vec<u8>>)>,
}

impl Dock {
    pub fn new() -> Self {
        Self {
            apps: Vec::new(),
            icons: HashMap::new(),
            pending: None,
            next_refresh: Instant::now(),
            last_foreground: 0,
        }
    }

    pub fn opened(&mut self, foreground: isize) {
        self.last_foreground = foreground;
        self.next_refresh = Instant::now();
    }

    pub fn refresh(&mut self, window: &TaskbarPreview, now: Instant) {
        if let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(snapshot) => {
                    self.pending = None;
                    if snapshot.apps != self.apps {
                        let keys: HashSet<_> = snapshot.apps.iter().map(IconKey::from).collect();
                        self.icons.retain(|key, _| keys.contains(key));
                        self.icons
                            .extend(snapshot.icons.into_iter().map(|(key, bytes)| {
                                let size = desktop::DOCK_ICON_SIZE;
                                let icon = bytes.map_or_else(Image::default, |bytes| {
                                    Image::from_rgba8(SharedPixelBuffer::clone_from_slice(
                                        &bytes, size, size,
                                    ))
                                });
                                (key, icon)
                            }));
                        window.set_live_apps(ModelRc::new(VecModel::from(
                            snapshot
                                .apps
                                .iter()
                                .map(|app| {
                                    let icon = self
                                        .icons
                                        .get(&IconKey::from(app))
                                        .cloned()
                                        .unwrap_or_default();
                                    view_model(app, icon)
                                })
                                .collect::<Vec<_>>(),
                        )));
                        self.apps = snapshot.apps;
                    }
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => self.pending = None,
            }
        }
        if now >= self.next_refresh {
            self.next_refresh = now + Duration::from_secs(1);
            let foreground = desktop::foreground_window_handle();
            if self
                .apps
                .iter()
                .flat_map(|app| &app.windows)
                .any(|window| window.handle == foreground as i64)
            {
                self.last_foreground = foreground;
            }
            let (sender, receiver) = mpsc::channel();
            let foreground = self.last_foreground;
            let mut known_icons: HashSet<_> = self.icons.keys().cloned().collect();
            if std::thread::Builder::new()
                .spawn(move || {
                    let apps = desktop::taskbar_dock(foreground);
                    let icons = apps
                        .iter()
                        .map(IconKey::from)
                        .filter(|key| known_icons.insert(key.clone()))
                        .map(|key| {
                            let pixels = desktop::dock_icon(&key.path, key.index);
                            (key, pixels)
                        })
                        .collect();
                    let _ = sender.send(Snapshot { apps, icons });
                })
                .is_ok()
            {
                self.pending = Some(receiver);
            }
        }
    }

    pub fn action_for(&self, target: &str) -> Option<(String, Action)> {
        if let Some(app) = self.apps.iter().find(|app| app.id == target) {
            let action = if let Some(window) = app.windows.first() {
                Action::Activate(window.clone())
            } else {
                Action::Open(app.launch_target.clone()?)
            };
            return Some((app.name.clone(), action));
        }
        let key = target.strip_prefix("dock-window:")?;
        let (pid, handle) = key.split_once(':')?;
        let (pid, handle) = (pid.parse::<u32>().ok()?, handle.parse::<i64>().ok()?);
        self.apps
            .iter()
            .flat_map(|app| &app.windows)
            .find(|window| window.process_id == pid && window.handle == handle)
            .map(|window| (window.title.clone(), Action::Activate(window.clone())))
    }
}

fn view_model(app: &NativeApp, icon: Image) -> DockApp {
    DockApp {
        id: app.id.as_str().into(),
        name: app.name.as_str().into(),
        icon,
        mark: app.name.chars().take(1).collect::<String>().into(),
        tint: slint::Color::from_rgb_u8(62, 90, 128),
        windows: i32::try_from(app.windows.len()).unwrap_or(i32::MAX),
        window_list: ModelRc::new(VecModel::from(
            app.windows.iter().map(window_model).collect::<Vec<_>>(),
        )),
        active: app.active,
        ..Default::default()
    }
}

fn window_model(window: &NativeWindow) -> DockWindow {
    DockWindow {
        key: format!("dock-window:{}:{}", window.process_id, window.handle).into(),
        title: window.title.as_str().into(),
    }
}
