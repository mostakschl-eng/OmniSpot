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
        .map(|app| format!("{} ({})", app.display_name, app.switches))
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
                    "Productivity streak",
                    if metrics.is_session_warmup {
                        format!("Warming up · {} / 10 switches", metrics.total_switch_count)
                    } else {
                        format!(
                            "{} days · {} today",
                            metrics.teleport_streak, metrics.today_switch_count
                        )
                    },
                ))
                .child(metric(
                    "Muscle memory",
                    if metrics.is_session_warmup {
                        "Measuring...".into()
                    } else {
                        format!(
                            "{} · {}",
                            metrics.muscle_memo_grade, metrics.muscle_memo_persona
                        )
                    },
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
        .child(metric(
            "Most peeked app",
            metrics
                .most_peeked_app
                .as_ref()
                .map(|app| format!("{} · {} peeks", app.display_name, app.peeks))
                .unwrap_or_else(|| "—".into()),
        ))
        .child(metric(
            "Shortcut maintenance",
            if metrics.is_maintenance_warmup {
                format!(
                    "Warming up · {} / 14 days",
                    metrics.maintenance_days.min(14)
                )
            } else {
                metrics
                    .first_stale_shortcut
                    .as_ref()
                    .map(|app| format!("Unused: {} ({})", app.display_name, app.letter))
                    .unwrap_or_else(|| "All shortcuts in use".into())
            },
        ))
        .child(metric("Total peeks", metrics.total_peek_count.to_string()))
}
