#[path = "CLI/mod.rs"]
mod cli;
#[path = "Overlay/mod.rs"]
mod overlay;
#[path = "Startup/mod.rs"]
mod startup;
#[path = "UI/mod.rs"]
mod ui;

use cli::cli_handler::{CliHandler, CliOutcome};
use cli::cli_options::CliOptions;
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};

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
    Application::new().run(move |cx: &mut App| {
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
                    cx.new(|_| ui::windows::spotlight::SpotlightWindow::default())
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
                    cx.new(|_| ui::SettingsWindow::default())
                })
                .expect("GPUI could not open Settings");
            }
        }
        cx.activate(true);
    });
}
