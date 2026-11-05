use crate::stats::StatsMetrics;
use gpui::{IntoElement, div, prelude::*, px, rgb};

fn metric(label: impl Into<String>, value: impl Into<String>) -> impl IntoElement {
    div()
        .flex_1()
        .min_h(px(110.0))
        .p_4()
        .rounded_md()
        .bg(rgb(0x303532))
        .flex()
        .flex_col()
        .justify_between()
        .child(div().text_xl().child(value.into()))
        .child(
            div()
                .text_sm()
                .text_color(rgb(0xc1c4c2))
                .child(label.into()),
        )
}

pub(crate) fn bento_dashboard(metrics: &StatsMetrics) -> impl IntoElement {
    let top_apps = metrics
        .podium
        .iter()
        .map(|app| format!("{} ({})", app.process_name, app.switches))
        .collect::<Vec<_>>()
        .join("  •  ");
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .gap_3()
                .child(metric("Life gained back", metrics.life_gained.clone()))
                .child(metric(
                    "Switches today",
                    metrics.today_switch_count.to_string(),
                ))
                .child(metric(
                    "Muscle memory",
                    format!(
                        "{} · {}",
                        metrics.muscle_memo_grade, metrics.muscle_memo_persona
                    ),
                )),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .child(metric(
                    "MOST USED APPS",
                    if top_apps.is_empty() {
                        "—".into()
                    } else {
                        top_apps
                    },
                ))
                .child(metric(
                    "Alt-Tab relapse",
                    format!(
                        "{}% · {} wasted",
                        metrics.alt_tab_relapse_pct, metrics.wasted_keystrokes
                    ),
                )),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .child(metric("Average latency", metrics.average_latency.clone()))
                .child(metric(
                    "Average glance",
                    format!(
                        "{} · sober {}d",
                        metrics.average_glance, metrics.alt_tab_sober_streak
                    ),
                )),
        )
        .child(metric("Total peeks", metrics.total_peek_count.to_string()))
}
