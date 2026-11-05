use gpui::{IntoElement, div, prelude::*, px, rgb};

fn metric(label: &'static str, value: &'static str) -> impl IntoElement {
    div()
        .flex_1()
        .min_h(px(110.0))
        .p_4()
        .rounded_md()
        .bg(rgb(0x303532))
        .flex()
        .flex_col()
        .justify_between()
        .child(div().text_xl().child(value))
        .child(div().text_sm().text_color(rgb(0xc1c4c2)).child(label))
}

pub(crate) fn bento_dashboard() -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .gap_3()
                .child(metric("Life gained back", "—"))
                .child(metric("Productivity streak", "—"))
                .child(metric("Muscle Memory Index", "—")),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .child(metric("MOST USED APPS · LAST 30D", "—"))
                .child(metric("HABIT TRACKER · LAST 30D", "—")),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .child(metric("SPEED DEMON · Avg Latency", "—"))
                .child(metric("PEEK STORY · Avg Glance", "—")),
        )
        .child(metric("DISCOVERY GAP · LAST 30D", "—"))
}
