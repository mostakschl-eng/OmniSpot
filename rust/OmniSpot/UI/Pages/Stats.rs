use crate::ui::SettingsWindow;
use crate::ui::controls::bento_dashboard::bento_dashboard;
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};

impl SettingsWindow {
    pub(crate) fn stats(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shield = div()
            .w_full()
            .h(px(440.0))
            .rounded_lg()
            .bg(rgb(0x2b302d))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(div().text_2xl().child("Privacy Shield"))
            .child(
                div()
                    .text_color(rgb(0xc1c4c2))
                    .child("Usage statistics are off by default"),
            )
            .child(
                div()
                    .w(px(380.0))
                    .p_4()
                    .rounded_md()
                    .bg(rgb(0x363b38))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child("Privacy Pledge")
                    .child("All usage data is stored locally on your device only.")
                    .child("Nothing is ever transmitted, sold, or shared.")
                    .child("You can disable collection at any time."),
            )
            .child(
                div()
                    .id("enable-statistics")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0xc6b690))
                    .text_color(rgb(0x171a18))
                    .cursor_pointer()
                    .child("Enable Statistics")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.stats_enabled = true;
                        cx.notify();
                    })),
            );

        div()
            .id("stats-scroll")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .child("Statistics")
            .child(if self.state.stats_enabled {
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .id("disable-statistics")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x414744))
                            .cursor_pointer()
                            .child("Disable Data Collection")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.stats_enabled = false;
                                cx.notify();
                            })),
                    )
                    .child(bento_dashboard())
                    .into_any_element()
            } else {
                shield.into_any_element()
            })
    }
}
