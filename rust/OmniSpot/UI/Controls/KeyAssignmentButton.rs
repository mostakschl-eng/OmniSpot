use gpui::{IntoElement, div, prelude::*, px, rgb};

pub(crate) fn key_assignment_button(key: &str) -> impl IntoElement {
    div()
        .w(px(30.0))
        .h(px(30.0))
        .rounded_md()
        .bg(rgb(0x414744))
        .flex()
        .items_center()
        .justify_center()
        .child(key.to_owned())
}
