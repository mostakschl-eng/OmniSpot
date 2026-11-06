use crate::ui::view_models::settings_state::SettingsState;
use async_channel::Sender;
use gpui::{
    Context, FocusHandle, IntoElement, KeyDownEvent, Render, Window, div, prelude::*, px, rgb,
};
use std::path::Path;
use std::sync::{Arc, Mutex};
use windows::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNO, MessageBoxW,
};
use windows::core::w;

const SHELL: u32 = 0x1c211f;
const PANEL: u32 = 0x232826;
const CARD: u32 = 0x303532;
const TEXT: u32 = 0xf4f5f4;
const MUTED: u32 = 0xc1c4c2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    General,
    Applications,
    Overlay,
    Spotlight,
    Stats,
    About,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Applications => "Applications",
            Self::Overlay => "Overlay",
            Self::Spotlight => "Spotlight Search",
            Self::Stats => "Stats",
            Self::About => "About",
        }
    }
}

pub struct SettingsWindow {
    page: Page,
    pub(crate) state: SettingsState,
    pub(crate) theme_menu_open: bool,
    pub(crate) add_flyout_open: bool,
    pub(crate) stats: Option<Arc<Mutex<crate::stats::StatsService>>>,
    pub(crate) events: Option<Sender<crate::input::hook::InputEvent>>,
    pub(crate) save_status: Option<(String, bool)>,
    pub(crate) available_apps: Vec<(String, String, Option<String>)>,
    pub(crate) assigning_key: Option<usize>,
    key_focus_handle: Option<FocusHandle>,
}

impl Default for SettingsWindow {
    fn default() -> Self {
        Self {
            page: Page::Applications,
            state: SettingsState::default(),
            theme_menu_open: false,
            add_flyout_open: false,
            stats: None,
            events: None,
            save_status: None,
            available_apps: Vec::new(),
            assigning_key: None,
            key_focus_handle: None,
        }
    }
}

pub(crate) fn running_applications() -> Vec<(String, String, Option<String>)> {
    let mut packages = crate::window_discovery::packaged_apps::PackagedApps::default();
    let installed = packages.installed().to_vec();
    crate::window_discovery::window_enumerator::WindowEnumerator::default()
        .get_windows()
        .into_iter()
        .filter(|window| !window.process_name().eq_ignore_ascii_case("OmniSpot.exe"))
        .map(|window| {
            let package = packages
                .contains_path(&window.process_image_path)
                .then(|| {
                    installed.iter().find(|package| {
                        Path::new(&window.process_image_path)
                            .starts_with(Path::new(&package.installed_path))
                    })
                })
                .flatten();
            let display_name = Path::new(&window.process_image_path)
                .file_stem()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| window.process_name().to_string());
            (
                display_name,
                window.process_image_path,
                package.and_then(|package| package.aumid.clone()),
            )
        })
        .fold(Vec::new(), |mut results, candidate| {
            if !results
                .iter()
                .any(|(_, path, _): &(String, String, Option<String>)| {
                    path.eq_ignore_ascii_case(&candidate.1)
                })
            {
                results.push(candidate);
            }
            results
        })
}
impl SettingsWindow {
    pub(crate) fn with_stats(
        stats: Option<Arc<Mutex<crate::stats::StatsService>>>,
        events: Option<Sender<crate::input::hook::InputEvent>>,
    ) -> Self {
        Self {
            stats,
            events,
            ..Self::default()
        }
    }
    fn nav(&self, page: Page, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.page == page;
        div()
            .id(page.title())
            .h(px(38.0))
            .px_3()
            .flex()
            .items_center()
            .rounded_md()
            .bg(rgb(if selected { 0x29312e } else { SHELL }))
            .hover(|style| style.bg(rgb(0x303835)))
            .cursor_pointer()
            .child(page.title())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.page = page;
                cx.notify();
            }))
    }

    pub(crate) fn toggle(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let supported = index != 2 || crate::ui::pages::general::pulse_border_supported();
        let enabled = match index {
            0 => self.state.launch_at_startup,
            1 => self.state.dynamic_mode,
            2 => self.state.pulse_border,
            3 => self.state.peek_mode,
            4 => self.state.overlay_enabled,
            5 => self.state.overlay_keep_open,
            6 => self.state.stats_enabled,
            7 => self.state.spotlight_enabled,
            _ => false,
        };
        div()
            .id(("toggle", index))
            .w(px(76.0))
            .ml_3()
            .px_2()
            .py_2()
            .rounded_md()
            .bg(rgb(if enabled { 0xc6b690 } else { 0x414744 }))
            .text_color(rgb(if enabled { 0x171a18 } else { TEXT }))
            .when(supported, |element| element.cursor_pointer())
            .child(if enabled { "●  On" } else { "○  Off" })
            .on_click(cx.listener(move |this, _, _, cx| {
                if index == 2 && !crate::ui::pages::general::pulse_border_supported() {
                    return;
                }
                match index {
                    0 => {
                        let _ = this
                            .state
                            .set_launch_at_startup(!this.state.launch_at_startup);
                    }
                    1 => this.state.dynamic_mode = !this.state.dynamic_mode,
                    2 => this.state.pulse_border = !this.state.pulse_border,
                    3 => this.state.peek_mode = !this.state.peek_mode,
                    4 => this.state.overlay_enabled = !this.state.overlay_enabled,
                    5 => this.state.overlay_keep_open = !this.state.overlay_keep_open,
                    6 => this.state.stats_enabled = !this.state.stats_enabled,
                    7 => this.state.spotlight_enabled = !this.state.spotlight_enabled,
                    _ => return,
                }
                if index == 4
                    && let Some(events) = &this.events
                {
                    let _ = events.try_send(crate::input::hook::InputEvent::OverlayPreview(
                        this.state.overlay_enabled,
                    ));
                }
                cx.notify();
            }))
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let key_focus_handle = self
            .key_focus_handle
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        let settings = cx.entity();
        _window.on_window_should_close(cx, move |_, cx| {
            settings.update(cx, |this, _| {
                if !this.state.is_dirty() {
                    return true;
                }
                let discard = unsafe {
                    MessageBoxW(
                        None,
                        w!("You have unsaved changes. Closing this window will discard them."),
                        w!("Unsaved Changes"),
                        MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
                    ) == IDYES
                };
                if discard {
                    let _ = this.state.reset();
                }
                discard
            })
        });
        let sidebar = div()
            .w(px(210.0))
            .h_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_1()
            .bg(rgb(SHELL))
            .child(self.nav(Page::General, cx))
            .child(self.nav(Page::Applications, cx))
            .child(self.nav(Page::Overlay, cx))
            .child(self.nav(Page::Spotlight, cx))
            .child(self.nav(Page::Stats, cx))
            .child(div().flex_1())
            .child(div().h(px(1.0)).bg(rgb(0x454946)))
            .child(self.nav(Page::About, cx));

        let content = div()
            .flex_1()
            .h_full()
            .p_5()
            .bg(rgb(PANEL))
            .child(match self.page {
                Page::General => self.general(cx).into_any_element(),
                Page::Applications => self
                    .applications(cx, key_focus_handle.clone())
                    .into_any_element(),
                Page::Overlay => self.overlay(cx).into_any_element(),
                Page::Spotlight => self.spotlight_settings(cx).into_any_element(),
                Page::Stats => self.stats(cx).into_any_element(),
                Page::About => crate::ui::pages::about::render(cx).into_any_element(),
            });

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(SHELL))
            .text_color(rgb(TEXT))
            .text_sm()
            .track_focus(&key_focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if let Some(index) = this.assigning_key {
                    let key = event.keystroke.key.to_ascii_uppercase();
                    if key == "ESCAPE" {
                        this.assigning_key = None;
                    } else if key.len() == 1 && key.as_bytes()[0].is_ascii_uppercase() {
                        if let Some(app) = this.state.applications.get_mut(index) {
                            app.key = key;
                        }
                        this.assigning_key = None;
                    }
                    cx.notify();
                }
            }))
            .child(
                div()
                    .h(px(50.0))
                    .px_5()
                    .flex()
                    .items_center()
                    .child("OmniSpot"),
            )
            .child(div().flex_1().flex().child(sidebar).child(content))
            .child(
                div()
                    .h(px(50.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .text_color(rgb(MUTED))
                    .child(
                        div()
                            .id("reset-changes")
                            .px_3()
                            .py_2()
                            .bg(rgb(CARD))
                            .cursor_pointer()
                            .child("Reset changes")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.save_status = this.state.reset().err().map(|error| (error, false));
                                if let Some(events) = &this.events {
                                    let _ = events.try_send(crate::input::hook::InputEvent::OverlayPreview(
                                        this.state.overlay_enabled,
                                    ));
                                }
                                cx.notify();
                            })),
                    )
                    .when_some(self.save_status.as_ref(), |element, (message, success)| {
                        element.child(
                            div()
                                .text_color(rgb(if *success { 0x9ad9a8 } else { 0xffa6a6 }))
                                .child(message.clone()),
                        )
                    })
                    .child(
                        div()
                            .id("save-apply")
                            .px_3()
                            .py_2()
                            .bg(rgb(if self.state.can_save() { 0xc6b690 } else { 0x414744 }))
                            .text_color(rgb(if self.state.can_save() { 0x171a18 } else { MUTED }))
                            .when(self.state.can_save(), |element| element.cursor_pointer())
                            .child("Save & Apply")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.state.can_save() {
                                    match this.state.save() {
                                        Ok(configuration) => {
                                            if let Some(events) = &this.events {
                                                let _ = events.try_send(
                                                    crate::input::hook::InputEvent::ConfigurationUpdated(configuration),
                                                );
                                            }
                                            this.save_status = Some(("Settings saved and applied.".to_string(), true));
                                        }
                                        Err(error) => this.save_status = Some((error, false)),
                                    }
                                }
                                cx.notify();
                            })),
                    ),
            )
    }
}
