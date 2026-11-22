#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use omnispot_spotlight_native::desktop;
use slint::ComponentHandle;

mod floating;
mod host;
mod icons;
mod settings;
mod taskbar_preview;

slint::include_modules!();

fn create_about() -> Result<AboutWindow, slint::PlatformError> {
    let window = AboutWindow::new()?;
    brand_window(window.window());
    window.set_version(env!("CARGO_PKG_VERSION").into());

    let weak = window.as_weak();
    window.on_open_website(move || {
        if let Some(window) = weak.upgrade() {
            window.set_feedback(open_website());
        }
    });

    let weak = window.as_weak();
    window.on_open_log_folder(move || {
        if let Some(window) = weak.upgrade() {
            window.set_feedback(open_log_folder());
        }
    });

    let weak = window.as_weak();
    window.on_copy_diagnostics(move || {
        if let Some(window) = weak.upgrade() {
            window.set_feedback(copy_diagnostics());
        }
    });

    Ok(window)
}

pub(crate) fn brand_window(window: &slint::Window) {
    if let Some(handle) = host::hwnd(window) {
        desktop::set_window_icon(handle);
    }
}

pub(crate) fn drag_window(window: &slint::Window) {
    if let Some(handle) = host::hwnd(window) {
        desktop::drag_window(handle);
    }
}

fn open_website() -> slint::SharedString {
    desktop::open_in_shell("https://app-switcher.com")
        .err()
        .map(|error| format!("Could not open the website. {error}"))
        .unwrap_or_default()
        .into()
}

fn open_log_folder() -> slint::SharedString {
    desktop::open_log_folder()
        .err()
        .map(|error| format!("Could not open the log folder. {error}"))
        .unwrap_or_default()
        .into()
}

fn copy_diagnostics() -> slint::SharedString {
    let (count, csv) = desktop::window_diagnostics();
    match clipboard_win::set_clipboard_string(&csv) {
        Ok(()) => format!("Copied diagnostics for {count} windows."),
        Err(error) => format!("Could not copy window diagnostics. {error}"),
    }
    .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == taskbar_preview::TEST_WORKER_ARG)
    {
        if let Err(error) = desktop::test_hide_windows_taskbar() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    run().inspect_err(|error| {
        if !cfg!(debug_assertions) {
            desktop::show_startup_error(&error.to_string());
        }
    })
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let directory = exe
        .parent()
        .ok_or("Could not locate the application directory")?;
    let preview = directory.join("slint-preview");
    let settings_path = preview.join("settings.sqlite3");
    match std::env::args_os().nth(1) {
        Some(option) if option == "--log-all-windows" => desktop::log_window_diagnostics()?,
        Some(option) if option == "--enable-auto-start" => desktop::set_startup(true)?,
        Some(option) if option == "--help" => println!(
            "OmniSpot [--run [--profile-dir PATH] | --settings-preview | --taskbar-preview | --migrate-profile PATH | --log-all-windows | --enable-auto-start]"
        ),
        Some(option) if option == "--migrate-profile" => {
            let mut args = std::env::args_os().skip(2);
            let path = args.next().ok_or("--migrate-profile requires a path")?;
            if args.next().is_some() {
                return Err("Expected --migrate-profile PATH".into());
            }
            let profile = std::path::PathBuf::from(path);
            let _instance = desktop::DesktopInstance::acquire()?;
            desktop::load_settings(&profile.join("settings.sqlite3"))?;
            desktop::import_stats(&profile.join("stats.sqlite3"))?;
        }
        Some(option) if option == "--run" => {
            let mut args = std::env::args_os().skip(2);
            let profile = match args.next() {
                None => desktop::default_profile_dir()?,
                Some(flag) if flag == "--profile-dir" => {
                    let path = args.next().ok_or("--profile-dir requires a path")?;
                    if args.next().is_some() { return Err("Unexpected application arguments".into()); }
                    path.into()
                }
                _ => return Err("Expected --run [--profile-dir PATH]".into()),
            };
            host::run(profile)?;
        }
        Some(option) if option == "--about" => create_about()?.run()?,
        Some(option) if option == "--taskbar-preview" => taskbar_preview::run()?,
        Some(option) if option == "--spotlight" => {
            let config = desktop::load_settings(&settings_path)?;
            let index = desktop::spotlight_index_path(&preview, &config);
            floating::create_spotlight(config, index)?.run()?;
        }
        Some(option) if option == "--overlay" => floating::create_overlay(desktop::load_settings(&settings_path)?)?.run()?,
        Some(option) if option == "--warning" => floating::create_warning(false)?.run()?,
        Some(option) if option == "--warning-restricted" => floating::create_warning(true)?.run()?,
        Some(option) if option == "--tray" => {
            let settings = settings::create(settings_path, false, |_| Ok(()))?;
            let tray = OmniSpotTray::new()?;
            let weak = settings.as_weak();
            tray.on_settings(move || {
                if let Some(settings) = weak.upgrade() { let _ = settings.show(); }
            });
            tray.on_exit_app(|| { let _ = slint::quit_event_loop(); });
            tray.show()?;
            slint::run_event_loop_until_quit()?;
        }
        Some(option) if option == "--profile-dir" => {
            let path = std::env::args_os()
                .nth(2)
                .ok_or("--profile-dir requires a path")?;
            settings::create(
                std::path::PathBuf::from(path).join("settings.sqlite3"),
                false,
                |_| Ok(()),
            )?.run()?;
        }
        Some(option) if option == "--settings-preview" => {
            settings::create(settings_path, false, |_| Ok(()))?.run()?;
        }
        None => host::run(desktop::default_profile_dir()?)?,
        Some(_) => return Err("Expected --run, --settings-preview, --taskbar-preview, --migrate-profile PATH, --about, --spotlight, --overlay, --warning, --warning-restricted, --tray or --profile-dir PATH".into()),
    }
    Ok(())
}
