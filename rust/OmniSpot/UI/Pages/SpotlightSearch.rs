use crate::ui::SettingsWindow;
use crate::ui::controls::modifier_key_selector::modifier_key_selector;
use crate::ui::controls::settings_card::settings_card;
use gpui::{Context, IntoElement, div, prelude::*};

impl SettingsWindow {
    pub(crate) fn spotlight_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("spotlight-settings-scroll")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .child("Spotlight Search")
            .child(settings_card(
                "Enable Spotlight Search",
                "Quick search launcher for applications and Windows Search indexed files.",
                self.toggle(7, cx),
            ))
            .child("Activation Shortcut (Modifier + Space bar)")
            .child(modifier_key_selector(
                &self.state.spotlight_modifier,
                true,
                cx,
            ))
    }
}
