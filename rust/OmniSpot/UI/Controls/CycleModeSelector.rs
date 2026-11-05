use crate::ui::SettingsWindow;
use crate::ui::converters::cycle_mode_to_label_converter::cycle_mode_to_label;
use crate::ui::view_models::application_shortcut::CycleMode;
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};

pub(crate) fn cycle_mode_selector(
    selected: &CycleMode,
    app_index: usize,
    cx: &mut Context<SettingsWindow>,
) -> impl IntoElement {
    div().flex().gap_1().children(
        [CycleMode::NextApp, CycleMode::Hide, CycleMode::NextWindow]
            .into_iter()
            .enumerate()
            .map(|(mode_index, mode)| {
                let active = *selected == mode;
                div()
                    .id(("cycle-mode", app_index * 3 + mode_index))
                    .h(px(20.0))
                    .px_2()
                    .rounded_sm()
                    .bg(rgb(if active { 0xc6b690 } else { 0x414744 }))
                    .text_color(rgb(if active { 0x171a18 } else { 0xf4f5f4 }))
                    .cursor_pointer()
                    .child(cycle_mode_to_label(&mode))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(app) = this.state.applications.get_mut(app_index) {
                            app.cycle_mode = mode;
                            cx.notify();
                        }
                    }))
            }),
    )
}
