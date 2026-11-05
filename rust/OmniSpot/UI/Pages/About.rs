use crate::ui::controls::settings_card::settings_card;
use crate::ui::view_models::about::AboutViewModel;
use gpui::{IntoElement, div, prelude::*, px, rgb};

pub(crate) fn render() -> impl IntoElement {
    div()
        .id("about-scroll")
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .w_full()
                .min_h(px(100.0))
                .p_4()
                .rounded_md()
                .bg(rgb(0x303532))
                .flex()
                .items_center()
                .gap_5()
                .child(div().w(px(52.0)).h(px(52.0)).rounded_md().bg(rgb(0x414744)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_xl().child(AboutViewModel::APP_NAME))
                        .child(AboutViewModel::app_version())
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0xc1c4c2))
                                .child(AboutViewModel::RUNTIME),
                        ),
                ),
        )
        .child("Support")
        .child(settings_card(
            "Log folder",
            "Open the folder containing application logs",
            div().child("Open"),
        ))
        .child(settings_card("Website", AboutViewModel::APP_WEBSITE, div()))
        .child(settings_card(
            "Window diagnostics",
            "Copy information about all open windows to the clipboard",
            div().child("Copy"),
        ))
}
