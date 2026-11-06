use super::session::{AppUsageStats, DailyBucket};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUsageSummary {
    pub process_name: String,
    pub display_name: String,
    pub switches: i32,
    pub peeks: i32,
    pub total_peek_time_ms: i32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleShortcutInfo {
    pub letter: String,
    pub process_name: String,
    pub display_name: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsMetrics {
    pub life_gained: String,
    pub total_switch_count: i32,
    pub total_peek_count: i32,
    pub average_glance: String,
    pub average_latency: String,
    pub static_percentage: i32,
    pub today_switch_count: i32,
    pub teleport_streak: i32,
    pub podium: Vec<AppUsageSummary>,
    pub most_peeked_app: Option<AppUsageSummary>,
    pub first_stale_shortcut: Option<StaleShortcutInfo>,
    pub is_maintenance_warmup: bool,
    pub maintenance_days: usize,
    pub is_session_warmup: bool,
    pub muscle_memo_grade: String,
    pub muscle_memo_persona: String,
    pub alt_tab_relapse_pct: i32,
    pub alt_tab_sober_streak: i32,
    pub wasted_keystrokes: i32,
    pub personal_best: Option<super::session::FastestSwitchRecord>,
}

pub struct StatsCalculator;
impl StatsCalculator {
    pub fn compute(
        all: &[DailyBucket],
        today: &DailyBucket,
        configured_static_apps: &[(String, String, String)],
    ) -> StatsMetrics {
        let today_index = day_index(&today.date);
        let recent = all
            .iter()
            .filter(|bucket| {
                let age = today_index - day_index(&bucket.date);
                (0..=30).contains(&age)
            })
            .collect::<Vec<_>>();
        let mut combined: HashMap<String, AppUsageStats> = HashMap::new();
        for bucket in recent.iter().copied().chain(std::iter::once(today)) {
            for (name, stats) in bucket
                .static_app_usage
                .iter()
                .chain(bucket.dynamic_app_usage.iter())
            {
                let aggregate = combined.entry(name.clone()).or_default();
                aggregate.switches += stats.switches;
                aggregate.peeks += stats.peeks;
                aggregate.total_peek_time_ms += stats.total_peek_time_ms;
                aggregate.total_switch_time_ms += stats.total_switch_time_ms;
            }
        }
        let total_switches = recent
            .iter()
            .map(|bucket| bucket.total_switches)
            .sum::<i32>()
            + today.total_switches;
        let is_maintenance_warmup = all.len() < 14;
        let is_session_warmup = total_switches < 10;
        let total_peeks =
            recent.iter().map(|bucket| bucket.total_peeks).sum::<i32>() + today.total_peeks;
        let total_saved = all
            .iter()
            .map(|bucket| bucket.total_time_saved_ms as i64)
            .sum::<i64>()
            + today.total_time_saved_ms as i64;
        let static_switches = recent
            .iter()
            .map(|bucket| {
                bucket
                    .static_app_usage
                    .values()
                    .map(|stats| stats.switches)
                    .sum::<i32>()
            })
            .sum::<i32>()
            + today
                .static_app_usage
                .values()
                .map(|stats| stats.switches)
                .sum::<i32>();
        let dynamic_switches = recent
            .iter()
            .map(|bucket| {
                bucket
                    .dynamic_app_usage
                    .values()
                    .map(|stats| stats.switches)
                    .sum::<i32>()
            })
            .sum::<i32>()
            + today
                .dynamic_app_usage
                .values()
                .map(|stats| stats.switches)
                .sum::<i32>();
        let total_glance_ms = combined
            .values()
            .map(|stats| stats.total_peek_time_ms as i64)
            .sum::<i64>();
        let total_latency_ms = combined
            .values()
            .map(|stats| stats.total_switch_time_ms as i64)
            .sum::<i64>();
        let most_peeked_app = combined
            .iter()
            .filter(|(_, stats)| stats.peeks > 0)
            .max_by_key(|(_, stats)| stats.peeks)
            .map(|(name, stats)| app_summary(name, stats, configured_static_apps));
        let first_stale_shortcut = if is_maintenance_warmup {
            None
        } else {
            configured_static_apps
                .iter()
                .find_map(|(name, letter, display_name)| {
                    let used = recent
                        .iter()
                        .copied()
                        .chain(std::iter::once(today))
                        .any(|bucket| {
                            bucket.static_app_usage.iter().any(|(key, stats)| {
                                key.eq_ignore_ascii_case(name) && stats.switches > 0
                            })
                        });
                    (!used).then(|| StaleShortcutInfo {
                        letter: letter.clone(),
                        process_name: name.clone(),
                        display_name: display_name.clone(),
                    })
                })
        };
        let mut podium = combined
            .into_iter()
            .map(|(name, stats)| app_summary(&name, &stats, configured_static_apps))
            .collect::<Vec<_>>();
        podium.sort_by_key(|app| std::cmp::Reverse(app.switches));
        podium.truncate(3);
        StatsMetrics {
            life_gained: format_duration(total_saved),
            total_switch_count: total_switches,
            total_peek_count: total_peeks,
            average_glance: if total_peeks == 0 {
                "—".to_string()
            } else {
                format!(
                    "{:.1}s",
                    total_glance_ms as f64 / total_peeks as f64 / 1000.0
                )
            },
            average_latency: if total_switches == 0 || total_latency_ms == 0 {
                "—".to_string()
            } else {
                format!("{}ms", total_latency_ms / total_switches as i64)
            },
            static_percentage: if static_switches + dynamic_switches == 0 {
                0
            } else {
                (static_switches as f64 * 100.0 / (static_switches + dynamic_switches) as f64)
                    .round_ties_even() as i32
            },
            today_switch_count: today.total_switches,
            teleport_streak: teleport_streak(all, today),
            podium,
            most_peeked_app,
            first_stale_shortcut,
            is_maintenance_warmup,
            maintenance_days: all.len(),
            is_session_warmup,
            muscle_memo_grade: muscle_memo(&recent, today)
                .map(|result| result.0)
                .unwrap_or_else(|| "—".to_string()),
            muscle_memo_persona: muscle_memo(&recent, today)
                .map(|result| result.1)
                .unwrap_or_else(|| "Measuring...".to_string()),
            alt_tab_relapse_pct: relapse_percentage(&recent, today),
            alt_tab_sober_streak: sober_streak(all, today),
            wasted_keystrokes: wasted_keystrokes(&recent, today),
            personal_best: (!is_session_warmup)
                .then(|| personal_best(all, today))
                .flatten(),
        }
    }
}

fn app_summary(
    name: &str,
    stats: &AppUsageStats,
    configured: &[(String, String, String)],
) -> AppUsageSummary {
    let display_name = configured
        .iter()
        .find(|(process, _, _)| process.eq_ignore_ascii_case(name))
        .map(|(_, _, display)| display.clone())
        .unwrap_or_else(|| name.strip_suffix(".exe").unwrap_or(name).to_string());
    AppUsageSummary {
        process_name: name.to_string(),
        display_name,
        switches: stats.switches,
        peeks: stats.peeks,
        total_peek_time_ms: stats.total_peek_time_ms,
    }
}

fn muscle_memo(all: &[&DailyBucket], today: &DailyBucket) -> Option<(String, String)> {
    let static_switches = all
        .iter()
        .copied()
        .chain(std::iter::once(today))
        .map(|bucket| {
            bucket
                .static_app_usage
                .values()
                .map(|stats| stats.switches)
                .sum::<i32>()
        })
        .sum::<i32>();
    let dynamic_switches = all
        .iter()
        .copied()
        .chain(std::iter::once(today))
        .map(|bucket| {
            bucket
                .dynamic_app_usage
                .values()
                .map(|stats| stats.switches)
                .sum::<i32>()
        })
        .sum::<i32>();
    let relapses = all
        .iter()
        .map(|bucket| bucket.alt_tab_switches)
        .sum::<i32>()
        + today.alt_tab_switches;
    let total = static_switches + dynamic_switches + relapses;
    if total == 0 {
        return None;
    }
    let index = (static_switches as f64 + dynamic_switches as f64 * 0.7) / total as f64 * 100.0;
    let (grade, persona) = match index {
        value if value >= 96.0 => ("S", "Shadow Walker"),
        value if value >= 85.0 => ("A", "Teleporter"),
        value if value >= 70.0 => ("B", "The Navigator"),
        value if value >= 50.0 => ("C", "Learner"),
        value if value >= 30.0 => ("D", "Novice"),
        _ => ("F", "Alt-Tabber"),
    };
    Some((grade.to_string(), persona.to_string()))
}

fn relapse_percentage(all: &[&DailyBucket], today: &DailyBucket) -> i32 {
    let alt = all
        .iter()
        .map(|bucket| bucket.alt_tab_switches)
        .sum::<i32>()
        + today.alt_tab_switches;
    let switches =
        all.iter().map(|bucket| bucket.total_switches).sum::<i32>() + today.total_switches;
    let total = alt + switches;
    if total == 0 {
        0
    } else {
        ((alt as f64 / total as f64) * 100.0).round() as i32
    }
}

fn wasted_keystrokes(all: &[&DailyBucket], today: &DailyBucket) -> i32 {
    all.iter()
        .copied()
        .chain(std::iter::once(today))
        .map(|bucket| (bucket.alt_tab_keystrokes - bucket.alt_tab_switches).max(0))
        .sum()
}

fn sober_streak(all: &[DailyBucket], today: &DailyBucket) -> i32 {
    let mut expected = day_index(&today.date) - 1;
    let mut streak = 0;
    for bucket in all.iter().rev() {
        if day_index(&bucket.date) != expected {
            break;
        }
        let total = bucket.total_switches + bucket.alt_tab_switches;
        if total > 0 && bucket.total_switches as f64 / total as f64 <= 0.95 {
            break;
        }
        streak += 1;
        expected -= 1;
    }
    streak
}

fn teleport_streak(all: &[DailyBucket], today: &DailyBucket) -> i32 {
    let mut expected = day_index(&today.date) - 1;
    let mut streak = 0;
    for bucket in all.iter().rev() {
        if day_index(&bucket.date) != expected || bucket.total_switches < 20 {
            break;
        }
        streak += 1;
        expected -= 1;
    }
    streak
}

fn personal_best(
    all: &[DailyBucket],
    today: &DailyBucket,
) -> Option<super::session::FastestSwitchRecord> {
    all.iter()
        .chain(std::iter::once(today))
        .filter_map(|bucket| bucket.fastest_switch.clone())
        .min_by_key(|record| record.duration_ms)
}

fn day_index(date: &str) -> i64 {
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return -1;
    };
    let (Ok(mut year), Ok(month), Ok(day)) = (
        year.parse::<i64>(),
        month.parse::<i64>(),
        day.parse::<i64>(),
    ) else {
        return -1;
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return -1;
    }
    year -= i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era
}
fn format_duration(milliseconds: i64) -> String {
    if milliseconds <= 0 {
        return "—".to_string();
    }
    let seconds = milliseconds / 1000;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        let minutes = seconds / 60;
        if seconds % 60 == 0 {
            format!("{minutes}m")
        } else {
            format!("{minutes}m {}s", seconds % 60)
        }
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn computes_dashboard_totals() {
        let mut today = DailyBucket::today();
        today.record_switch(
            "notepad.exe",
            100,
            200,
            false,
            Some(100),
            crate::configuration::Key::N,
        );
        today.record_peek("notepad.exe", 400, false);
        let metrics = StatsCalculator::compute(&[], &today, &[]);
        assert_eq!(metrics.total_switch_count, 1);
        assert_eq!(metrics.total_peek_count, 1);
        assert_eq!(metrics.static_percentage, 100);
        assert_eq!(metrics.podium[0].process_name, "notepad.exe");
        assert_eq!(
            metrics.most_peeked_app.as_ref().unwrap().display_name,
            "notepad"
        );
        assert!(metrics.is_session_warmup);
    }

    #[test]
    fn parses_csharp_local_calendar_dates_for_streaks() {
        assert_eq!(day_index("2026-11-06") - day_index("2026-11-05"), 1);
        assert_eq!(day_index("2024-03-01") - day_index("2024-02-29"), 1);
        assert_eq!(day_index("invalid"), -1);
    }

    #[test]
    fn dashboard_rounds_and_formats_like_csharp() {
        assert_eq!(format_duration(61_000), "1m 1s");
        assert_eq!(format_duration(3_601_000), "1h 0m");

        let mut today = DailyBucket::today();
        today.static_app_usage.insert(
            "static.exe".into(),
            AppUsageStats {
                switches: 1,
                ..Default::default()
            },
        );
        today.dynamic_app_usage.insert(
            "dynamic.exe".into(),
            AppUsageStats {
                switches: 1,
                ..Default::default()
            },
        );
        assert_eq!(
            StatsCalculator::compute(&[], &today, &[]).static_percentage,
            50
        );
    }

    #[test]
    fn sober_streak_requires_more_than_95_percent_non_alt_tab_switches() {
        let mut yesterday = DailyBucket::today();
        yesterday.date = "2026-11-05".into();
        yesterday.total_switches = 19;
        yesterday.alt_tab_switches = 1;
        let mut today = DailyBucket::today();
        today.date = "2026-11-06".into();
        assert_eq!(sober_streak(&[yesterday.clone()], &today), 0);

        yesterday.total_switches = 20;
        assert_eq!(sober_streak(&[yesterday], &today), 1);
    }

    #[test]
    fn productivity_streak_counts_consecutive_days_with_twenty_switches() {
        let mut yesterday = DailyBucket::today();
        yesterday.date = "2026-11-05".into();
        yesterday.total_switches = 20;
        let mut two_days_ago = DailyBucket::today();
        two_days_ago.date = "2026-11-04".into();
        two_days_ago.total_switches = 25;
        let mut today = DailyBucket::today();
        today.date = "2026-11-06".into();
        assert_eq!(teleport_streak(&[two_days_ago, yesterday], &today), 2);
    }

    #[test]
    fn lifetime_saved_time_keeps_old_buckets_while_dashboard_totals_use_30_days() {
        let mut old = DailyBucket::today();
        old.date = "2026-10-06".into();
        old.total_switches = 40;
        old.total_time_saved_ms = 61_000;
        old.static_app_usage.insert(
            "old.exe".into(),
            AppUsageStats {
                switches: 40,
                ..Default::default()
            },
        );
        let mut today = DailyBucket::today();
        today.date = "2026-11-06".into();
        today.total_switches = 2;
        today.static_app_usage.insert(
            "today.exe".into(),
            AppUsageStats {
                switches: 2,
                ..Default::default()
            },
        );

        let metrics = StatsCalculator::compute(&[old], &today, &[]);
        assert_eq!(metrics.total_switch_count, 2);
        assert_eq!(metrics.podium[0].process_name, "today.exe");
        assert_eq!(metrics.life_gained, "1m 1s");
        assert!(metrics.personal_best.is_none());
    }

    #[test]
    fn maintenance_warmup_hides_stale_shortcuts_until_fourteen_history_days() {
        let today = DailyBucket::today();
        let apps = vec![("unused.exe".into(), "U".into(), "Unused App".into())];
        let early = StatsCalculator::compute(&[], &today, &apps);
        assert!(early.is_maintenance_warmup);
        assert!(early.first_stale_shortcut.is_none());

        let mut history = (1..=14)
            .map(|day| {
                let mut bucket = DailyBucket::today();
                bucket.date = format!("2026-10-{day:02}");
                bucket
            })
            .collect::<Vec<_>>();
        history.sort_by(|a, b| a.date.cmp(&b.date));
        let ready = StatsCalculator::compute(&history, &today, &apps);
        assert!(!ready.is_maintenance_warmup);
        let stale = ready.first_stale_shortcut.unwrap();
        assert_eq!(stale.letter, "U");
        assert_eq!(stale.display_name, "Unused App");
    }
}
