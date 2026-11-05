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
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x292e2b))
                .child("Search installed applications"),
        )
        .child("Choose an application to assign a shortcut")
        .child(
            div()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x414744))
                .child("Browse for executable"),
        )
}
