use crate::{
    floating, settings, AppOverlayWindow, OmniSpotTray, SpotlightWindow, WarningOverlayWindow,
};
use omnispot_spotlight_native::desktop::{
    self, CaptureEvent, Configuration, DesktopCapture, DesktopInstance,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{ComponentHandle, Timer, TimerMode};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

struct Windows {
    config: Configuration,
    overlay: Option<AppOverlayWindow>,
    spotlight: Option<SpotlightWindow>,
    warning: Option<WarningOverlayWindow>,
    overlay_deadline: Option<Instant>,
}

impl Windows {
    fn hide_overlay(&mut self) {
        if let Some(window) = self.overlay.take() {
            let _ = window.hide();
        }
    }

    fn toggle_spotlight(&mut self, index: &Path) -> Result<(), slint::PlatformError> {
        if self.spotlight.is_none() {
            self.spotlight = Some(floating::create_spotlight(
                self.config.clone(),
                index.to_owned(),
            )?);
        }
        if let Some(window) = &self.spotlight {
            if window.window().is_visible() {
                window.hide()?;
            } else {
                window.set_query(Default::default());
                window.invoke_search(Default::default());
                place(window.window(), true);
                window.show()?;
                place(window.window(), true);
                if let Some(handle) = hwnd(window.window()) {
                    desktop::focus_window(handle);
                }
            }
        }
        Ok(())
    }

    fn reconfigure(&mut self, config: Configuration) {
        self.hide_overlay();
        if let Some(spotlight) = self.spotlight.take() {
            let _ = spotlight.hide();
        }
        self.overlay_deadline = None;
        self.config = config;
    }
}

pub(crate) fn hwnd(window: &slint::Window) -> Option<isize> {
    let handle = window.window_handle();
    match handle.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

fn place(window: &slint::Window, spotlight: bool) {
    let size = window.size();
    if let Some((x, y)) =
        desktop::place_popup(size.width, size.height, window.scale_factor(), spotlight)
    {
        window.set_position(slint::PhysicalPosition::new(x, y));
    }
}

fn show_overlay(
    windows: &mut Windows,
    apps: Vec<desktop::ApplicationConfiguration>,
    dynamic: bool,
) -> Result<(), slint::PlatformError> {
    windows.hide_overlay();
    let config = Configuration {
        applications: apps,
        dynamic_mode_enabled: dynamic,
        ..windows.config.clone()
    };
    let overlay = floating::create_overlay(config)?;
    if let Some(handle) = hwnd(overlay.window()) {
        desktop::set_passive_window(handle);
    }
    place(overlay.window(), false);
    overlay.show()?;
    place(overlay.window(), false);
    windows.overlay = Some(overlay);
    Ok(())
}

pub fn run(profile: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let settings_path = profile.join("settings.sqlite3");
    let stats_path = profile.join("stats.sqlite3");
    let _instance = DesktopInstance::acquire()?;
    let config = desktop::load_settings(&settings_path)?;
    desktop::import_stats(&stats_path)?;
    let windows = Rc::new(RefCell::new(Windows {
        config,
        overlay: None,
        spotlight: None,
        warning: None,
        overlay_deadline: None,
    }));
    let state = windows.clone();
    let stats = stats_path.clone();
    let startup_interactive =
        desktop::default_profile_dir().is_ok_and(|default| default == profile);
    let settings = settings::create(settings_path, startup_interactive, move |config| {
        DesktopCapture::update(config.clone());
        let stats_enabled = config.stats_enabled;
        state.borrow_mut().reconfigure(config);
        desktop::update_stats(&stats, stats_enabled)
    })?;
    let tray = OmniSpotTray::new()?;
    let weak = settings.as_weak();
    tray.on_settings(move || {
        if let Some(settings) = weak.upgrade() {
            let _ = settings.show();
        }
    });
    tray.on_exit_app(|| {
        let _ = slint::quit_event_loop();
    });
    tray.show()?;
    if let Err(error) = desktop::update_stats(&stats_path, windows.borrow().config.stats_enabled) {
        settings.set_feedback(error.into());
    }

    let tick = Timer::default();
    let index_dir = profile;
    let weak = settings.as_weak();
    let mut last_warning = None::<Instant>;
    let mut next_flush = Instant::now() + Duration::from_secs(300);
    let stats = stats_path.clone();
    let (capture, events) = DesktopCapture::install(windows.borrow().config.clone())?;
    tick.start(TimerMode::Repeated, Duration::from_millis(16), move || {
        let Ok(mut windows) = windows.try_borrow_mut() else {
            return;
        };
        let now = Instant::now();
        for event in events.try_iter().take(64) {
            let result = match event {
                CaptureEvent::StartOverlayTimer(delay) => {
                    windows.overlay_deadline = now.checked_add(delay);
                    Ok(())
                }
                CaptureEvent::CancelOverlayTimer => {
                    windows.overlay_deadline = None;
                    Ok(())
                }
                CaptureEvent::ShowOverlay(apps, dynamic) => {
                    show_overlay(&mut windows, apps, dynamic)
                }
                CaptureEvent::HideOverlay => {
                    windows.hide_overlay();
                    Ok(())
                }
                CaptureEvent::ToggleSpotlight => {
                    let index = desktop::spotlight_index_path(&index_dir, &windows.config);
                    windows.toggle_spotlight(&index)
                }
                CaptureEvent::Warn { restricted } => {
                    if let Some(warning) = windows.warning.take() {
                        let _ = warning.hide();
                    }
                    floating::create_warning(restricted).and_then(|warning| {
                        warning
                            .global::<crate::OmniTheme>()
                            .set_requested_theme(windows.config.theme as i32);
                        if let Some(handle) = hwnd(warning.window()) {
                            desktop::set_passive_window(handle);
                        }
                        place(warning.window(), false);
                        warning.show()?;
                        place(warning.window(), false);
                        windows.warning = Some(warning);
                        last_warning = Some(now);
                        Ok(())
                    })
                }
                CaptureEvent::PotentialElevation => {
                    capture.potential_elevation_detected();
                    Ok(())
                }
                CaptureEvent::Error(error) => {
                    if let Some(settings) = weak.upgrade() {
                        settings.set_feedback(error.into());
                    }
                    Ok(())
                }
            };
            if let Err(error) = result {
                if let Some(settings) = weak.upgrade() {
                    settings.set_feedback(error.to_string().into());
                }
            }
        }
        if windows
            .overlay_deadline
            .is_some_and(|deadline| now >= deadline)
        {
            windows.overlay_deadline = None;
            capture.show_overlay_if_enabled();
        }
        capture.set_overlay_visible(
            windows
                .overlay
                .as_ref()
                .is_some_and(|window| window.window().is_visible()),
        );
        if let Some(at) = last_warning {
            let progress = (1.0 - now.duration_since(at).as_secs_f32() / 5.0).max(0.0);
            if progress == 0.0 {
                if let Some(warning) = windows.warning.take() {
                    let _ = warning.hide();
                }
                last_warning = None;
            } else if let Some(warning) = &windows.warning {
                warning.set_countdown(progress);
            }
        }
        if now >= next_flush {
            next_flush = now + Duration::from_secs(300);
            if windows.config.stats_enabled {
                if let Err(error) = desktop::flush_stats(&stats) {
                    if let Some(settings) = weak.upgrade() {
                        settings.set_feedback(error.into());
                    }
                }
            }
        }
    });
    let result = slint::run_event_loop_until_quit();
    tick.stop();
    let flushed = desktop::update_stats(&stats_path, false);
    result?;
    flushed?;
    Ok(())
}
