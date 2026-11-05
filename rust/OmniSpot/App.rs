#[path = "ApplicationLifecycle.rs"]
mod application_lifecycle;
#[path = "CLI/mod.rs"]
mod cli;
#[path = "Configuration/mod.rs"]
mod configuration;
#[path = "Input/mod.rs"]
mod input;
#[path = "Logging.rs"]
mod logging;
#[path = "Overlay/mod.rs"]
mod overlay;
#[path = "Spotlight/mod.rs"]
mod spotlight;
#[path = "Startup/mod.rs"]
mod startup;
#[path = "Stats/mod.rs"]
mod stats;
#[path = "Switcher.rs"]
mod switcher;
#[path = "UI/mod.rs"]
mod ui;
#[path = "WindowDiscovery/mod.rs"]
mod window_discovery;

use cli::cli_handler::{CliHandler, CliOutcome};
use cli::cli_options::CliOptions;
use gpui::{
    App, AppContext, Application, Bounds, WindowBackgroundAppearance, WindowBounds, WindowKind,
    WindowOptions, point, px, size,
};
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver},
};
use std::time::Duration;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let screen = if args.len() == 1
        && matches!(
            args[0].as_str(),
            "--ui-spotlight" | "--ui-overlay" | "--ui-warning"
        ) {
        args[0].clone()
    } else {
        let builder = cli::service_collection_extensions::cli_builder();
        let mut options = CliOptions::default();
        let outcome = CliHandler { builder: &builder }.handle(&args, &mut options);
        logging::initialize(options.enable_debug_logging, options.enable_trace_logging);
        match outcome {
            Ok(CliOutcome::Continue) => {}
            Ok(CliOutcome::Help(message)) => {
                println!("{message}");
                return;
            }
            Ok(CliOutcome::Command(command)) => {
                if let Err(error) = command() {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
                return;
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        }
        String::new()
    };
    if screen.is_empty() {
        let single_instance = match application_lifecycle::SingleInstance::acquire() {
            Ok(instance) => instance,
            Err(error) => {
                eprintln!("{error}");
                return;
            }
        };
        let executable_dir = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let portable = executable_dir.join(".portable").exists();
        let settings_path = match configuration::SettingsPath::resolve(&executable_dir, portable) {
            Ok(path) => path,
            Err(error) => {
                eprintln!("Could not determine settings path: {error:?}");
                std::process::exit(1);
            }
        };
        let mut manager = configuration::ConfigurationManager::new(
            configuration::ConfigurationService::new(settings_path),
        );
        let config = match manager.load() {
            Ok(config) => config,
            Err(error) => {
                eprintln!("Could not load configuration: {error:?}");
                std::process::exit(1);
            }
        };
        let (event_sender, event_receiver) = mpsc::channel();
        let tray = ui::tray::TrayIcon::spawn(event_sender.clone()).ok();
        let mut stats_service = stats::StatsService::new(
            std::env::var_os("LOCALAPPDATA")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("OmniSpot")
                .join("stats.json"),
        );
        stats_service.start(config.stats_enabled);
        let stats_service = Arc::new(Mutex::new(stats_service));
        let ui_configuration = config.clone();
        let controller =
            input::hook::InputController::new_with_events(config, Some(event_sender.clone()));
        let input_controller = controller.clone();
        let input_thread = input::hook::InputHook::spawn(controller);
        run_application(
            screen,
            Some(single_instance),
            Some(input_thread),
            Some(event_receiver),
            Some(stats_service),
            Some(ui_configuration),
            tray,
            Some(event_sender),
            Some(input_controller),
        );
        return;
    }
    run_application(screen, None, None, None, None, None, None, None, None);
}

fn run_application(
    screen: String,
    _single_instance: Option<application_lifecycle::SingleInstance>,
    input_thread: Option<input::hook::InputThread>,
    event_receiver: Option<Receiver<input::hook::InputEvent>>,
    stats_service: Option<Arc<Mutex<stats::StatsService>>>,
    ui_configuration: Option<configuration::Configuration>,
    tray: Option<ui::tray::TrayIcon>,
    action_sender: Option<std::sync::mpsc::Sender<input::hook::InputEvent>>,
    input_controller: Option<Arc<Mutex<input::hook::InputController>>>,
) {
    let dimensions = match screen.as_str() {
        "" => (900.0, 770.0),
        "--ui-spotlight" => (540.0, 380.0),
        "--ui-overlay" => (340.0, 260.0),
        "--ui-warning" => (420.0, 180.0),
        _ => {
            eprintln!("Usage: omnispot-rust [--ui-spotlight|--ui-overlay|--ui-warning]");
            std::process::exit(2);
        }
    };
    let stats_for_events = stats_service.clone();
    let stats_for_window = stats_service.clone();
    let action_sender_for_overlay = action_sender.clone();
    Application::new().run(move |cx: &mut App| {
        let _tray = &tray;
        let _lifecycle = (&input_thread, &_single_instance);
        if let Some(event_receiver) = event_receiver {
            cx.spawn(move |cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                let mut ui_configuration = ui_configuration.clone();
                async move {
                    let mut overlay_window = None;
                    loop {
                        while let Ok(event) = event_receiver.try_recv() {
                            match event {
                                input::hook::InputEvent::OpenSpotlight => {
                                    let bounds = Bounds::new(
                                        point(px(100.0), px(100.0)),
                                        size(px(540.0), px(380.0)),
                                    );
                                    let options = WindowOptions {
                                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                                        ..Default::default()
                                    };
                                    let _ = cx.open_window(options, |window, cx| {
                                        window.set_window_title("OmniSpot Spotlight UI");
                                        cx.new(|_| ui::windows::spotlight::SpotlightWindow::new())
                                    });
                                }
                                input::hook::InputEvent::OpenSettings => {
                                    let bounds = Bounds::new(
                                        point(px(100.0), px(100.0)),
                                        size(px(900.0), px(770.0)),
                                    );
                                    let options = WindowOptions {
                                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                                        ..Default::default()
                                    };
                                    let _ = cx.open_window(options, |window, cx| {
                                        window.set_window_title("OmniSpot Settings");
                                        cx.new(|_| {
                                            ui::SettingsWindow::with_stats(
                                                stats_for_events.clone(),
                                                action_sender_for_overlay.clone(),
                                            )
                                        })
                                    });
                                }
                                input::hook::InputEvent::Exit => {
                                    let _ = cx.update(|cx| cx.quit());
                                }
                                input::hook::InputEvent::ActivateWindow { handle } => {
                                    switcher::Switcher::activate_handle(handle);
                                }
                                input::hook::InputEvent::LaunchApplication { path, aumid } => {
                                    if let Some(aumid) = aumid {
                                        let _ = std::process::Command::new("explorer.exe")
                                            .args(["shell:AppsFolder\\", aumid.as_str()])
                                            .spawn();
                                    } else {
                                        let _ = std::process::Command::new(path).spawn();
                                    }
                                }
                                input::hook::InputEvent::ConfigurationUpdated(configuration) => {
                                    if let Some(controller) = &input_controller {
                                        if let Ok(mut controller) = controller.lock() {
                                            controller.update_configuration(configuration.clone());
                                        }
                                    }
                                    if let Some(stats) = &stats_for_events {
                                        if let Ok(mut stats) = stats.lock() {
                                            stats.set_enabled(configuration.stats_enabled);
                                        }
                                    }
                                    ui_configuration = Some(configuration);
                                }
                                input::hook::InputEvent::Switch {
                                    process,
                                    duration_ms,
                                    choices,
                                    dynamic,
                                    key,
                                } => {
                                    if let Some(stats) = &stats_for_events {
                                        if let Ok(stats) = stats.lock() {
                                            stats.record_switch(
                                                &process,
                                                duration_ms,
                                                choices,
                                                dynamic,
                                                key,
                                            );
                                        }
                                    }
                                }
                                input::hook::InputEvent::AltTab { count } => {
                                    if let Some(stats) = &stats_for_events {
                                        if let Ok(stats) = stats.lock() {
                                            stats.record_alt_tab(count);
                                        }
                                    }
                                }
                                input::hook::InputEvent::Peek {
                                    process,
                                    duration_ms,
                                    dynamic,
                                } => {
                                    if let Some(stats) = &stats_for_events {
                                        if let Ok(stats) = stats.lock() {
                                            stats.record_peek(&process, duration_ms, dynamic);
                                        }
                                    }
                                }
                                input::hook::InputEvent::OverlayShow => {
                                    if overlay_window.is_none() {
                                        let state = ui_configuration
                                            .as_ref()
                                            .map(build_overlay_state)
                                            .unwrap_or_default();
                                        let bounds = overlay_bounds();
                                        let options = WindowOptions {
                                            window_bounds: Some(WindowBounds::Windowed(bounds)),
                                            titlebar: None,
                                            focus: false,
                                            kind: WindowKind::PopUp,
                                            is_movable: false,
                                            is_resizable: false,
                                            is_minimizable: false,
                                            window_background:
                                                WindowBackgroundAppearance::Transparent,
                                            ..Default::default()
                                        };
                                        overlay_window = cx
                                            .open_window(options, |window, cx| {
                                                window.set_window_title("OmniSpot Overlay UI");
                                                cx.new(|_| {
                                                    ui::windows::app_overlay::AppOverlayWindow {
                                                        state,
                                                        events: action_sender_for_overlay.clone(),
                                                    }
                                                })
                                            })
                                            .ok();
                                    }
                                }
                                input::hook::InputEvent::OverlayHide => {
                                    if let Some(window) = overlay_window.take() {
                                        let _ = window.update(&mut cx, |_, window, _| {
                                            window.remove_window();
                                        });
                                    }
                                }
                            }
                        }
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                    }
                }
            })
            .detach();
        }
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(dimensions.0), px(dimensions.1)), cx);
        let options = || WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        };
        match screen.as_str() {
            "--ui-spotlight" => {
                cx.open_window(options(), |window, cx| {
                    window.set_window_title("OmniSpot Spotlight UI");
                    cx.new(|_| ui::windows::spotlight::SpotlightWindow::new())
                })
                .expect("GPUI could not open Spotlight");
            }
            "--ui-overlay" => {
                cx.open_window(options(), |window, cx| {
                    window.set_window_title("OmniSpot Overlay UI");
                    cx.new(|_| ui::windows::app_overlay::AppOverlayWindow::default())
                })
                .expect("GPUI could not open Overlay");
            }
            "--ui-warning" => {
                cx.open_window(options(), |window, cx| {
                    window.set_window_title("OmniSpot Warning UI");
                    cx.new(|_| ui::windows::warning_overlay::WarningOverlayWindow {
                        content: overlay::warning_content::WarningContent::Elevated,
                        dismissed: false,
                    })
                })
                .expect("GPUI could not open Warning");
            }
            _ => {
                cx.open_window(options(), |window, cx| {
                    window.set_window_title("OmniSpot Settings");
                    cx.new(|_| {
                        ui::SettingsWindow::with_stats(
                            stats_for_window.clone(),
                            action_sender.clone(),
                        )
                    })
                })
                .expect("GPUI could not open Settings");
            }
        }
        cx.activate(true);
    });
    if let Some(stats) = stats_service {
        if let Ok(stats) = stats.lock() {
            stats.flush();
        }
    }
}

fn overlay_bounds() -> Bounds<gpui::Pixels> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    };

    let mut cursor = POINT::default();
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor);
    }
    let mut monitor = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let work_area = unsafe {
        let handle = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        if handle.0.is_null() || !GetMonitorInfoW(handle, &mut monitor).as_bool() {
            windows::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
                ..Default::default()
            }
        } else {
            monitor.rcWork
        }
    };
    let width = 340.0;
    let height = 260.0;
    let left = (cursor.x as f32 - width / 2.0).clamp(
        work_area.left as f32,
        (work_area.right as f32 - width).max(work_area.left as f32),
    );
    let below = cursor.y as f32 + 16.0;
    let top = if below + height <= work_area.bottom as f32 {
        below
    } else {
        cursor.y as f32 - height - 16.0
    }
    .clamp(
        work_area.top as f32,
        (work_area.bottom as f32 - height).max(work_area.top as f32),
    );
    Bounds::new(point(px(left), px(top)), size(px(width), px(height)))
}

fn build_overlay_state(
    config: &configuration::Configuration,
) -> ui::view_models::overlay::OverlayState {
    let applications = config
        .applications
        .iter()
        .map(ui::view_models::application_shortcut::ApplicationShortcut::from_configuration)
        .collect::<Vec<_>>();
    let mut enumerator = window_discovery::WindowEnumerator::default();
    let current_handle = enumerator.get_current_window().map(|window| window.handle);
    let discovered_windows = enumerator.get_windows();
    let mut app_registry = window_discovery::AppRegistryCache::for_user();
    app_registry.prepopulate(config, &discovered_windows);
    let dynamic_configurations = [
        configuration::Key::A,
        configuration::Key::B,
        configuration::Key::C,
        configuration::Key::D,
        configuration::Key::E,
        configuration::Key::F,
        configuration::Key::G,
        configuration::Key::H,
        configuration::Key::I,
        configuration::Key::J,
        configuration::Key::K,
        configuration::Key::L,
        configuration::Key::M,
        configuration::Key::N,
        configuration::Key::O,
        configuration::Key::P,
        configuration::Key::Q,
        configuration::Key::R,
        configuration::Key::S,
        configuration::Key::T,
        configuration::Key::U,
        configuration::Key::V,
        configuration::Key::W,
        configuration::Key::X,
        configuration::Key::Y,
        configuration::Key::Z,
    ]
    .into_iter()
    .flat_map(|key| {
        input::dynamic_mode_service::DynamicModeService::get_apps_for_key_with_registry(
            key,
            &config.applications,
            &discovered_windows,
            &mut app_registry,
        )
    })
    .collect::<Vec<_>>();
    let dynamic_apps = dynamic_configurations
        .iter()
        .map(ui::view_models::application_shortcut::ApplicationShortcut::from_configuration)
        .collect::<Vec<_>>();
    let windows = discovered_windows
        .into_iter()
        .into_iter()
        .map(|window| overlay::app_overlay_service::DiscoveredWindow {
            process_name: window.process_name().to_string(),
            process_path: window.process_image_path,
            title: window.title,
            handle: window.handle,
            needs_elevation: window.needs_elevation,
        })
        .collect::<Vec<_>>();
    let mut service = overlay::app_overlay_service::AppOverlayService::default();
    service.show(
        &applications,
        &windows,
        &dynamic_apps,
        config.dynamic_mode_enabled,
        current_handle,
    )
}
