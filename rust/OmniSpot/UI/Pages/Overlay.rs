use crate::ui::SettingsWindow;
use crate::ui::controls::settings_card::settings_card;
use gpui::{Context, IntoElement, div, prelude::*, rgb};

impl SettingsWindow {
    pub(crate) fn overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("overlay-scroll")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .child("App Switcher Overlay")
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0xc1c4c2))
                    .child("The overlay switch previews immediately. Save & Apply keeps your choice."),
            )
            .child(settings_card(
                "Show overlay",
                "Display a floating list of configured apps when the modifier key is held.",
                self.toggle(4, cx),
            ))
            .child(settings_card(
                "Show delay",
                "How long the modifier key must be held before the overlay appears.",
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("overlay-delay-decrease")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x414744))
                            .cursor_pointer()
                            .child("−")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.state.overlay_enabled {
                                    this.state.overlay_delay_ms = this.state.overlay_delay_ms.saturating_sub(100);
                                    cx.notify();
                                }
                            })),
                    )
                    .child(format!("{} ms", self.state.overlay_delay_ms))
                    .child(
                        div()
                            .id("overlay-delay-increase")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x414744))
                            .cursor_pointer()
                            .child("+")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.state.overlay_enabled {
                                    this.state.overlay_delay_ms = this.state.overlay_delay_ms.saturating_add(100).min(5000);
                                    cx.notify();
                                }
                            })),
                    ),
            ))
            .child(settings_card(
                "Keep open while modifier is held",
                "The overlay stays open and refreshes after each app switch until the modifier is released.",
                self.toggle(5, cx),
            ))
    }
}
