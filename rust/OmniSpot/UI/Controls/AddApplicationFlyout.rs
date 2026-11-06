use gpui::{IntoElement, div, prelude::*, px, rgb};

pub(crate) fn add_application_flyout() -> impl IntoElement {
    div()
        .w(px(390.0))
        .p_4()
        .rounded_lg()
        .bg(rgb(0x363b38))
        .flex()
        .flex_col()
        .gap_3()
        .child("Add application")
        .child("Choose a running application or browse for an executable.")
}
