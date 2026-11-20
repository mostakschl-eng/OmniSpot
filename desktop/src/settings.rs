use crate::{HotkeyDraft, OmniTheme, PickerApp, SettingsDraft, SettingsWindow, StatCard, StatDay};
use omnispot_spotlight_native::desktop::{
    self, AppThemeSetting, ApplicationConfiguration, ApplicationType, Configuration, CycleMode,
    FIRST_HOTKEY, MODIFIERS,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

pub fn create(
    path: PathBuf,
    startup_interactive: bool,
    applied: impl Fn(Configuration) -> Result<(), String> + 'static,
) -> Result<SettingsWindow, Box<dyn std::error::Error>> {
    let baseline = Rc::new(RefCell::new(desktop::load_settings(&path)?));
    let window = SettingsWindow::new()?;
    window.set_version(env!("CARGO_PKG_VERSION").into());
    window.set_editable(true);
    window.set_startup_enabled(desktop::startup_enabled());
    window.set_startup_interactive(startup_interactive);
    window.set_pulse_supported(desktop::pulse_supported());
    refresh(&window, &baseline.borrow());

    let weak = window.as_weak();
    let running = RefCell::new(Vec::new());
    window.on_filter_apps(move |query| {
        let Some(window) = weak.upgrade() else { return };
        let mut running = running.borrow_mut();
        if query.is_empty() || running.is_empty() {
            *running = desktop::running_app_paths()
                .into_iter()
                .map(|path| {
                    let name = desktop::application_display_name(&path);
                    (name, path)
                })
                .collect();
            running.sort_by_key(|(name, _)| name.to_lowercase());
        }
        let query = query.trim().to_lowercase();
        let draft = window.get_draft();
        let apps = running
            .iter()
            .filter(|(name, path)| {
                (name.to_lowercase().contains(&query) || path.to_lowercase().contains(&query))
                    && !draft
                        .applications
                        .iter()
                        .any(|app| app.path.eq_ignore_ascii_case(path))
            })
            .map(|(name, path)| PickerApp {
                name: name.as_str().into(),
                path: path.as_str().into(),
                icon: crate::icons::load(path),
            })
            .collect::<Vec<_>>();
        window.set_running_apps(ModelRc::new(VecModel::from(apps)));
    });
    let weak = window.as_weak();
    window.on_refresh_bindings(move || {
        let Some(window) = weak.upgrade() else { return };
        let config = apply_draft(&Configuration::default(), &window.get_draft());
        let apps = desktop::dynamic_applications(&config)
            .iter()
            .map(hotkey_draft)
            .collect::<Vec<_>>();
        window.set_dynamic_bindings(ModelRc::new(VecModel::from(apps)));
    });
    let weak = window.as_weak();
    window.on_pin_app(move |index| {
        let Some(window) = weak.upgrade() else { return };
        if let Some(app) = window.get_dynamic_bindings().row_data(index as usize) {
            change_applications(&window, |apps| apps.push(app));
        }
    });

    let weak = window.as_weak();
    window.on_open_website(move || {
        if let Some(window) = weak.upgrade() {
            window.set_feedback(crate::open_website());
        }
    });
    let weak = window.as_weak();
    window.on_open_log_folder(move || {
        if let Some(window) = weak.upgrade() {
            window.set_feedback(crate::open_log_folder());
        }
    });
    let weak = window.as_weak();
    window.on_copy_diagnostics(move || {
        if let Some(window) = weak.upgrade() {
            window.set_feedback(crate::copy_diagnostics());
        }
    });
    let weak = window.as_weak();
    window.on_docs(move |url| {
        if let (Some(window), Err(error)) = (weak.upgrade(), desktop::open_in_shell(url.as_str())) {
            window.set_feedback(error.into());
        }
    });
    let saved = baseline.clone();
    let weak = window.as_weak();
    window.on_edit(move || {
        if let Some(window) = weak.upgrade() {
            let draft = window.get_draft();
            let saved = saved.borrow();
            window.set_dirty(
                apply_draft(&saved, &draft) != *saved
                    || (startup_interactive
                        && window.get_startup_enabled() != desktop::startup_enabled()),
            );
            window
                .global::<OmniTheme>()
                .set_requested_theme(draft.theme);
            window.set_feedback(SharedString::default());
        }
    });
    let saved = baseline.clone();
    let weak = window.as_weak();
    window.on_reset(move || {
        if let Some(window) = weak.upgrade() {
            refresh(&window, &saved.borrow());
            desktop::DesktopCapture::preview_overlay(window.get_draft().overlay);
            window.set_startup_enabled(desktop::startup_enabled());
            window.invoke_refresh_bindings();
        }
    });
    window.on_preview_overlay(desktop::DesktopCapture::preview_overlay);
    let weak = window.as_weak();
    window.window().on_close_requested(move || {
        if let Some(window) = weak.upgrade() {
            if window.get_dirty() {
                window.set_confirm_close(true);
                return slint::CloseRequestResponse::KeepWindowShown;
            }
        }
        slint::CloseRequestResponse::HideWindow
    });
    let weak = window.as_weak();
    window.on_discard(move || {
        if let Some(window) = weak.upgrade() {
            window.invoke_reset();
            let _ = window.hide();
        }
    });
    let stats_db = path.with_file_name("stats.sqlite3");
    let saved = baseline;
    let weak = window.as_weak();
    window.on_save(move || {
        let Some(window) = weak.upgrade() else { return };
        let change = apply_draft(&saved.borrow(), &window.get_draft());
        match desktop::save_settings(&path, change.clone()) {
            Ok(()) => {
                *saved.borrow_mut() = change.clone();
                let startup = if startup_interactive
                    && window.get_startup_enabled() != desktop::startup_enabled()
                {
                    desktop::set_startup(window.get_startup_enabled())
                } else {
                    Ok(())
                };
                window.set_dirty(startup.is_err());
                let message = match applied(change).and(startup) {
                    Ok(()) => "Settings saved.".to_owned(),
                    Err(error) => format!("Settings saved. {error}"),
                };
                window.set_feedback(message.into());
            }
            Err(error) => window.set_feedback(error.into()),
        }
    });
    let weak = window.as_weak();
    window.on_add_folder(move || {
        let Some(window) = weak.upgrade() else { return };
        let Some(folder) = choose(&window, true) else {
            return;
        };
        let mut draft = window.get_draft();
        let mut folders = draft.spotlight_folders.iter().collect::<Vec<_>>();
        if !folders
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&folder))
        {
            folders.push(folder.into());
            draft.spotlight_folders = ModelRc::new(VecModel::from(folders));
            window.set_draft(draft);
            window.invoke_edit();
        }
    });
    let weak = window.as_weak();
    window.on_remove_folder(move |index| {
        let Some(window) = weak.upgrade() else { return };
        let mut draft = window.get_draft();
        let mut folders = draft.spotlight_folders.iter().collect::<Vec<_>>();
        if let Some(index) = usize::try_from(index)
            .ok()
            .filter(|&index| index < folders.len())
        {
            folders.remove(index);
            draft.spotlight_folders = ModelRc::new(VecModel::from(folders));
            window.set_draft(draft);
            window.invoke_edit();
        }
    });
    let weak = window.as_weak();
    window.on_browse_app(move || {
        if let Some(window) = weak.upgrade() {
            if let Some(path) = choose(&window, false) {
                add_application(&window, path.into(), false);
            }
        }
    });
    let weak = window.as_weak();
    window.on_add_app(move |path| {
        if let Some(window) = weak.upgrade() {
            add_application(&window, path, true);
        }
    });
    let weak = window.as_weak();
    window.on_edit_app(move |index, app| {
        if let Some(window) = weak.upgrade() {
            let rows = window.get_draft().applications;
            if (index as usize) < rows.row_count() {
                rows.set_row_data(index as usize, app);
                window.invoke_edit();
            }
        }
    });
    let weak = window.as_weak();
    window.on_record_app_key(move |index, text| {
        let (Some(window), [letter]) = (weak.upgrade(), text.as_bytes()) else {
            return false;
        };
        if !letter.is_ascii_alphabetic() {
            return false;
        }
        let rows = window.get_draft().applications;
        let Some(mut row) = rows.row_data(index as usize) else {
            return false;
        };
        row.key = i32::from(letter.to_ascii_uppercase() - b'A');
        rows.set_row_data(index as usize, row);
        window.invoke_edit();
        window.invoke_refresh_bindings();
        true
    });
    let weak = window.as_weak();
    window.on_move_app(move |index, delta| {
        if let Some(window) = weak.upgrade() {
            change_applications(&window, |apps| {
                let (index, target) = (index as usize, index.saturating_add(delta) as usize);
                if index < apps.len() && target < apps.len() {
                    apps.swap(index, target);
                }
            });
        }
    });
    let weak = window.as_weak();
    window.on_remove_app(move |index| {
        if let Some(window) = weak.upgrade() {
            change_applications(&window, |apps| {
                if (index as usize) < apps.len() {
                    apps.remove(index as usize);
                }
            });
        }
    });
    let weak = window.as_weak();
    let stats = stats_db.clone();
    window.on_refresh_stats(move || {
        if let Some(window) = weak.upgrade() {
            let configuration = apply_draft(&Configuration::default(), &window.get_draft());
            match desktop::stats_metrics(&stats, &configuration) {
                Ok(data) => {
                    window.set_stats_summary(if data.cards.is_empty() {
                        "No usage history yet.".into()
                    } else {
                        "Last 30 days".into()
                    });
                    window.set_stats_cards(ModelRc::new(VecModel::from(
                        data.cards
                            .into_iter()
                            .map(|(title, value, progress)| StatCard {
                                title: title.into(),
                                value: value.into(),
                                progress: progress.map_or(-1.0, |percent| percent as f32 / 100.0),
                            })
                            .collect::<Vec<_>>(),
                    )));
                    let max = data
                        .week
                        .iter()
                        .map(|(_, ms)| *ms)
                        .max()
                        .unwrap_or(0)
                        .max(1);
                    window.set_stats_week(ModelRc::new(VecModel::from(
                        data.week
                            .into_iter()
                            .map(|(day, ms)| StatDay {
                                day: day.into(),
                                saved: format!("{:.1}s", f64::from(ms) / 1000.0).into(),
                                height_ratio: ms as f32 / max as f32,
                            })
                            .collect::<Vec<_>>(),
                    )));
                }
                Err(error) => window.set_stats_summary(error.into()),
            }
        }
    });
    let weak = window.as_weak();
    window.on_reset_stats(move || {
        if let Some(window) = weak.upgrade() {
            match desktop::clear_stats(&stats_db) {
                Ok(()) => window.invoke_refresh_stats(),
                Err(error) => window.set_feedback(error.into()),
            }
        }
    });
    window.invoke_refresh_bindings();
    crate::brand_window(window.window());
    Ok(window)
}

fn choose(window: &SettingsWindow, directory: bool) -> Option<String> {
    match desktop::choose_path(directory) {
        Ok(value) => value,
        Err(error) => {
            window.set_feedback(error.into());
            None
        }
    }
}

fn add_application(window: &SettingsWindow, path: SharedString, running: bool) {
    let aumid = running
        .then(|| desktop::running_application_aumid(path.as_str()))
        .flatten();
    if aumid.is_none() && !desktop::is_executable(path.as_str()) {
        window.set_feedback("The selected file is not a valid Windows executable.".into());
        return;
    }
    change_applications(window, |apps| {
        if !apps
            .iter()
            .any(|app| app.path.eq_ignore_ascii_case(path.as_str()))
        {
            let key = (0..26)
                .find(|&key| !apps.iter().any(|app| app.key == key))
                .unwrap_or(0);
            apps.push(HotkeyDraft {
                icon: crate::icons::load(path.as_str()),
                name: desktop::application_display_name(path.as_str()).into(),
                path,
                key,
                packaged: aumid.is_some(),
                aumid: aumid.unwrap_or_default().into(),
                ..HotkeyDraft::default()
            });
        }
    });
}

fn change_applications(window: &SettingsWindow, edit: impl FnOnce(&mut Vec<HotkeyDraft>)) {
    let mut draft = window.get_draft();
    let mut apps = draft.applications.iter().collect::<Vec<_>>();
    edit(&mut apps);
    draft.applications = ModelRc::new(VecModel::from(apps));
    window.set_draft(draft);
    window.invoke_edit();
    window.invoke_refresh_bindings();
}

fn modifier_index(key: u32) -> i32 {
    MODIFIERS
        .iter()
        .position(|&value| value == key)
        .map_or(-1, |index| index as i32)
}

fn apply_draft(config: &Configuration, draft: &SettingsDraft) -> Configuration {
    let modifier = |index: i32, old| {
        usize::try_from(index)
            .ok()
            .and_then(|index| MODIFIERS.get(index))
            .copied()
            .unwrap_or(old)
    };
    let folders: Vec<_> = draft
        .spotlight_folders
        .iter()
        .map(|value| value.to_string())
        .collect();
    Configuration {
        theme: match draft.theme {
            1 => AppThemeSetting::Dark,
            2 => AppThemeSetting::Light,
            _ => AppThemeSetting::System,
        },
        modifier: modifier(draft.modifier, config.modifier),
        dynamic_mode_enabled: draft.dynamic_mode,
        pulse_border_enabled: draft.pulse_border,
        peek_enabled: draft.peek,
        overlay_enabled: draft.overlay,
        overlay_show_delay_ms: draft.overlay_delay,
        overlay_keep_open_while_modifier_held: draft.overlay_keep_open,
        spotlight_enabled: draft.spotlight,
        spotlight_modifier: modifier(draft.spotlight_modifier, config.spotlight_modifier),
        spotlight_custom_directories: (!folders.is_empty()).then_some(folders),
        stats_enabled: draft.stats,
        applications: draft
            .applications
            .iter()
            .map(|item| ApplicationConfiguration {
                process_path: item.path.to_string(),
                key: u32::try_from(item.key)
                    .ok()
                    .and_then(|key| FIRST_HOTKEY.checked_add(key))
                    .unwrap_or(0),
                cycle_mode: match item.cycle {
                    1 => CycleMode::Hide,
                    2 => CycleMode::NextWindow,
                    _ => CycleMode::NextApp,
                },
                start_if_not_running: item.start,
                application_type: if item.packaged {
                    ApplicationType::Packaged
                } else {
                    ApplicationType::Win32
                },
                aumid: (!item.aumid.is_empty()).then(|| item.aumid.to_string()),
            })
            .collect(),
    }
}

fn refresh(window: &SettingsWindow, config: &Configuration) {
    let theme = config.theme as i32;
    let folders = config
        .spotlight_custom_directories
        .iter()
        .flatten()
        .map(|item| item.as_str().into())
        .collect::<Vec<_>>();
    window.set_draft(SettingsDraft {
        theme,
        modifier: modifier_index(config.modifier),
        dynamic_mode: config.dynamic_mode_enabled,
        pulse_border: config.pulse_border_enabled,
        peek: config.peek_enabled,
        overlay: config.overlay_enabled,
        overlay_delay: config.overlay_show_delay_ms,
        overlay_keep_open: config.overlay_keep_open_while_modifier_held,
        spotlight: config.spotlight_enabled,
        spotlight_modifier: modifier_index(config.spotlight_modifier),
        spotlight_folders: ModelRc::new(VecModel::from(folders)),
        stats: config.stats_enabled,
        applications: ModelRc::new(VecModel::from(
            config
                .applications
                .iter()
                .map(hotkey_draft)
                .collect::<Vec<_>>(),
        )),
    });
    window.global::<OmniTheme>().set_requested_theme(theme);
    window.set_dirty(false);
    window.set_feedback(SharedString::default());
}

fn hotkey_draft(app: &ApplicationConfiguration) -> HotkeyDraft {
    HotkeyDraft {
        name: desktop::application_display_name(&app.process_path).into(),
        icon: crate::icons::load(&desktop::application_icon_path(app)),
        path: app.process_path.as_str().into(),
        key: app
            .key
            .checked_sub(FIRST_HOTKEY)
            .map_or(-1, |key| key as i32),
        cycle: app.cycle_mode as i32,
        start: app.start_if_not_running,
        packaged: app.application_type == ApplicationType::Packaged,
        aumid: app.aumid.as_deref().unwrap_or_default().into(),
    }
}
