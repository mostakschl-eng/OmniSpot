use crate::ui::view_models::settings_state::SettingsState;
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px, rgb};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

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
        }
    }
}
impl SettingsWindow {
    pub(crate) fn with_stats(
        stats: Option<Arc<Mutex<crate::stats::StatsService>>>,
        events: Option<Sender<crate::input::hook::InputEvent>>,
    ) -> Self {
        let mut settings = Self::default();
        settings.stats = stats;
        settings.events = events;
        settings
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
            .cursor_pointer()
            .child(if enabled { "●  On" } else { "○  Off" })
            .on_click(cx.listener(move |this, _, _, cx| {
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
                    6 => {
                        this.state.stats_enabled = !this.state.stats_enabled;
                        if let Some(stats) = &this.stats {
                            if let Ok(mut stats) = stats.lock() {
                                stats.set_enabled(this.state.stats_enabled);
                            }
                        }
                    }
                    7 => this.state.spotlight_enabled = !this.state.spotlight_enabled,
                    _ => return,
                }
                cx.notify();
            }))
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                Page::Applications => self.applications(cx).into_any_element(),
                Page::Overlay => self.overlay(cx).into_any_element(),
                Page::Spotlight => self.spotlight_settings(cx).into_any_element(),
                Page::Stats => self.stats(cx).into_any_element(),
                Page::About => crate::ui::pages::about::render().into_any_element(),
            });

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(SHELL))
            .text_color(rgb(TEXT))
            .text_sm()
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
                                let _ = this.state.reset();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("save-apply")
                            .px_3()
                            .py_2()
                            .bg(rgb(0xc6b690))
                            .text_color(rgb(0x171a18))
                            .cursor_pointer()
                            .child("Save & Apply")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Ok(configuration) = this.state.save() {
                                    if let Some(events) = &this.events {
                                        let _ = events.send(
                                            crate::input::hook::InputEvent::ConfigurationUpdated(
                                                configuration,
                                            ),
                                        );
                                    }
                                }
                                cx.notify();
                            })),
                    ),
            )
    }
}
