use crate::ui::view_models::spotlight::{SearchResult, SpotlightState};
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px, rgb};

#[derive(Default)]
pub struct SpotlightWindow {
    pub state: SpotlightState,
}

fn result_row(result: &SearchResult) -> impl IntoElement {
    div()
        .h(px(42.0))
        .px_3()
        .rounded_md()
        .flex()
        .items_center()
        .justify_between()
        .child(result.title.clone())
        .child(
            div()
                .text_xs()
                .text_color(rgb(0xabb3bb))
                .child(result.kind.clone()),
        )
}

impl Render for SpotlightWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_3()
            .rounded_lg()
            .bg(rgb(0x191d24))
            .text_color(rgb(0xf4f7fb))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(56.0))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .h(px(44.0))
                            .px_4()
                            .rounded_md()
                            .bg(rgb(0x242a33))
                            .flex()
                            .items_center()
                            .child(if self.state.query.is_empty() {
                                "⌕   Search apps and files".to_owned()
                            } else {
                                self.state.query.clone()
                            }),
                    )
                    .child("ESC"),
            )
            .child(div().h(px(1.0)).bg(rgb(0x43505a)))
            .child(
                div()
                    .flex_1()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_xs().child(if self.state.query.is_empty() {
                        "QUICK ACCESS"
                    } else {
                        "SEARCH RESULTS"
                    }))
                    .children(if self.state.query.is_empty() {
                        self.state
                            .suggestions
                            .iter()
                            .map(result_row)
                            .collect::<Vec<_>>()
                    } else {
                        self.state
                            .results
                            .iter()
                            .map(result_row)
                            .collect::<Vec<_>>()
                    }),
            )
            .child(div().h(px(1.0)).bg(rgb(0x43505a)))
            .child(
                div()
                    .h(px(42.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .child(self.state.selected_path.clone())
                    .child("↑↓  Select     Enter  Open"),
            )
    }
}
