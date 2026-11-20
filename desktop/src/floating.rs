use crate::{
    AppOverlayWindow, OmniTheme, OverlayRow, SearchRow, SpotlightWindow, WarningOverlayWindow,
};
use omnispot_spotlight_native::desktop::{self, Configuration, SpotlightSearch};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub fn create_spotlight(
    config: Configuration,
    path: PathBuf,
) -> Result<SpotlightWindow, slint::PlatformError> {
    let window = SpotlightWindow::new()?;
    window
        .global::<OmniTheme>()
        .set_requested_theme(config.theme as i32);
    window.set_feedback("Preparing search index…".into());
    let search = Arc::new(Mutex::new(None::<SpotlightSearch>));
    let index = search.clone();
    let weak = window.as_weak();
    window.on_search(move |query| {
        let Some(window) = weak.upgrade() else { return };
        let Ok(index) = index.lock() else { return };
        let Some(index) = index.as_ref() else { return };
        let rows = index
            .search(query.as_str())
            .into_iter()
            .map(|result| SearchRow {
                icon: crate::icons::load(&desktop::search_icon_path(&result)),
                title: result.title.into(),
                subtitle: result.subtitle.into(),
                target: result.target.into(),
                kind: result.result_type.into(),
            })
            .collect::<Vec<_>>();
        window.set_feedback(if rows.is_empty() && !query.trim().is_empty() {
            "No matching apps or files".into()
        } else {
            SharedString::default()
        });
        window.set_selected_index(0);
        window.set_results(ModelRc::new(VecModel::from(rows)));
    });
    let index = search.clone();
    let weak = window.as_weak();
    window.on_launch(move |row| {
        let Some(window) = weak.upgrade() else { return };
        let Some(result) = usize::try_from(row)
            .ok()
            .and_then(|row| window.get_results().row_data(row))
        else {
            return;
        };
        let Ok(mut index) = index.lock() else { return };
        let Some(index) = index.as_mut() else { return };
        match index.launch(result.target.as_str(), result.kind.as_str()) {
            Ok(()) => {
                let _ = window.hide();
            }
            Err(error) => window.set_feedback(error.into()),
        }
    });
    let weak = window.as_weak();
    window.on_dismiss(move || {
        if let Some(window) = weak.upgrade() {
            let _ = window.hide();
        }
    });
    let weak = window.as_weak();
    std::thread::spawn(move || {
        let index = SpotlightSearch::load(&config, path);
        if let Ok(mut target) = search.lock() {
            *target = Some(index);
        }
        let _ = weak.upgrade_in_event_loop(|window| window.invoke_search(window.get_query()));
    });
    Ok(window)
}

pub fn create_overlay(
    configuration: Configuration,
) -> Result<AppOverlayWindow, slint::PlatformError> {
    let window = AppOverlayWindow::new()?;
    window
        .global::<OmniTheme>()
        .set_requested_theme(configuration.theme as i32);
    let apps = desktop::overlay_items(&configuration);
    let empty_running_before = if apps.iter().any(|app| app.section == "Running apps") {
        None
    } else {
        apps.iter()
            .position(|app| matches!(app.section.as_str(), "Launchable apps" | "Dynamic apps"))
    };
    window.set_empty_running_before(empty_running_before.map_or(-1, |index| index as i32));
    let mut previous = "";
    let rows = apps
        .iter()
        .enumerate()
        .map(|(index, app)| {
            let section = if previous == app.section {
                String::new()
            } else if matches!(
                app.section.as_str(),
                "Running apps" | "Launchable apps" | "Dynamic apps"
            ) {
                app.section.clone()
            } else {
                format!("{} windows", app.section)
            };
            previous = &app.section;
            let section_gap =
                if section.is_empty() || (index == 0 && empty_running_before != Some(0)) {
                    0.0
                } else {
                    match app.section.as_str() {
                        "Running apps" => 28.0,
                        "Launchable apps" => 16.0,
                        "Dynamic apps" => 8.0,
                        _ => 12.0,
                    }
                };
            OverlayRow {
                icon: crate::icons::load(&app.icon_path),
                badge: app.badge.as_str().into(),
                active: app.active,
                elevated: app.elevated,
                section: section.into(),
                section_gap,
                label: app.label.as_str().into(),
            }
        })
        .collect::<Vec<_>>();
    let headers: f32 = rows
        .iter()
        .filter(|row| !row.section.is_empty())
        .map(|row| 21.0 + row.section_gap)
        .sum();
    window.set_rows_height(if rows.is_empty() {
        44.0
    } else {
        rows.len() as f32 * 32.0
            + headers
            + if empty_running_before.is_some() {
                40.0
            } else {
                0.0
            }
    });
    window.set_apps(ModelRc::new(VecModel::from(rows)));
    let weak = window.as_weak();
    window.on_open_app(move |index| {
        let (Some(window), Some(app)) = (weak.upgrade(), apps.get(index as usize)) else {
            return;
        };
        match app.activate() {
            Ok(()) => {
                let _ = window.hide();
            }
            Err(error) => window.set_feedback(error.into()),
        }
    });
    let weak = window.as_weak();
    window.on_dismiss(move || {
        if let Some(window) = weak.upgrade() {
            let _ = window.hide();
        }
    });
    Ok(window)
}

pub fn create_warning(restricted: bool) -> Result<WarningOverlayWindow, slint::PlatformError> {
    let window = WarningOverlayWindow::new()?;
    window.set_restricted(restricted);
    window.on_learn_more(|| {
        let _ = desktop::open_in_shell("https://app-switcher.com/elevated-apps/");
    });
    let weak = window.as_weak();
    window.on_dismiss(move || {
        if let Some(window) = weak.upgrade() {
            let _ = window.hide();
        }
    });
    Ok(window)
}
