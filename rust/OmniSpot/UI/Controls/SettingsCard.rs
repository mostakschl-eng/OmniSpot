use gpui::{IntoElement, div, prelude::*, px, rgb};

pub(crate) fn settings_card(
    title: &'static str,
    detail: &'static str,
    value: impl IntoElement,
) -> impl IntoElement {
    div()
        .w_full()
        .min_h(px(64.0))
        .px_5()
        .py_3()
        .rounded_md()
        .bg(rgb(0x303532))
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(title)
                .child(div().text_sm().text_color(rgb(0xc1c4c2)).child(detail)),
        )
        .child(value)
}
