use crate::ui::SettingsWindow;
use gpui::{Context, FocusHandle, IntoElement, div, prelude::*, px, rgb};

pub(crate) fn key_assignment_button(
    key: &str,
    index: usize,
    listening: bool,
    focus_handle: FocusHandle,
    cx: &mut Context<SettingsWindow>,
) -> impl IntoElement {
    let focus_handle = focus_handle.clone();
    div()
        .id(("assign-key", index))
        .w(px(30.0))
        .h(px(30.0))
        .rounded_md()
        .bg(rgb(0x414744))
        .flex()
        .items_center()
        .justify_center()
        .child(if listening {
            "...".to_string()
        } else if key == "None" {
            "?".to_string()
        } else {
            key.to_owned()
        })
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, window, cx| {
            this.assigning_key = Some(index);
            window.focus(&focus_handle);
            cx.notify();
        }))
}
