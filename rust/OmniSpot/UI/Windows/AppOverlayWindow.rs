use crate::ui::view_models::overlay::{OverlayRow, OverlayState};
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px, rgb};

#[derive(Default)]
pub struct AppOverlayWindow {
    pub state: OverlayState,
}

fn row(item: &OverlayRow) -> impl IntoElement {
    div()
        .h(px(32.0))
        .px_1()
        .rounded_md()
        .bg(rgb(if item.active { 0x3e4940 } else { 0x252a27 }))
        .flex()
        .items_center()
        .justify_between()
        .child(item.name.clone())
        .when(item.elevated, |element| element.child("⚠"))
        .child(
            div()
                .px_2()
                .py_1()
                .bg(rgb(0x414744))
                .child(item.key.clone()),
        )
}

impl Render for AppOverlayWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .bg(rgb(0x252a27))
            .text_color(rgb(0xf4f5f4))
            .text_sm()
            .flex()
            .flex_col()
            .gap_2()
            .children(self.state.window_groups.iter().map(|group| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(format!("{} windows", group.app_name))
                    .children(group.windows.iter().map(row))
            }))
            .child(div().text_xs().child("Running apps"))
            .children(self.state.running.iter().map(row))
            .when(self.state.running.is_empty(), |element| {
                element.child(
                    div()
                        .h(px(32.0))
                        .text_color(rgb(0xa3aaa5))
                        .child("No running apps"),
                )
            })
            .when(!self.state.launchable.is_empty(), |element| {
                element
                    .child(div().text_xs().child("Launchable apps"))
                    .children(self.state.launchable.iter().map(row))
            })
            .when(!self.state.dynamic.is_empty(), |element| {
                element
                    .child(div().text_xs().child("Dynamic apps"))
                    .children(self.state.dynamic.iter().map(row))
            })
    }
}
