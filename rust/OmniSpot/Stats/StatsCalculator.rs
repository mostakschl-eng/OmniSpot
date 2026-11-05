use super::session::{AppUsageStats, DailyBucket};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUsageSummary {
    pub process_name: String,
    pub switches: i32,
    pub peeks: i32,
    pub total_peek_time_ms: i32,
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
    pub podium: Vec<AppUsageSummary>,
    pub muscle_memo_grade: String,
    pub muscle_memo_persona: String,
    pub alt_tab_relapse_pct: i32,
    pub alt_tab_sober_streak: i32,
    pub wasted_keystrokes: i32,
    pub personal_best: Option<super::session::FastestSwitchRecord>,
}

pub struct StatsCalculator;
impl StatsCalculator {
    pub fn compute(all: &[DailyBucket], today: &DailyBucket) -> StatsMetrics {
        let mut combined: HashMap<String, AppUsageStats> = HashMap::new();
        for bucket in all.iter().chain(std::iter::once(today)) {
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
        let total_switches =
            all.iter().map(|bucket| bucket.total_switches).sum::<i32>() + today.total_switches;
        let total_peeks =
            all.iter().map(|bucket| bucket.total_peeks).sum::<i32>() + today.total_peeks;
        let total_saved = all
            .iter()
            .map(|bucket| bucket.total_time_saved_ms as i64)
            .sum::<i64>()
            + today.total_time_saved_ms as i64;
        let static_switches = all
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
        let dynamic_switches = all
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
        let mut podium = combined
            .into_iter()
            .map(|(process_name, stats)| AppUsageSummary {
                process_name,
                switches: stats.switches,
                peeks: stats.peeks,
                total_peek_time_ms: stats.total_peek_time_ms,
            })
            .collect::<Vec<_>>();
        podium.sort_by(|left, right| right.switches.cmp(&left.switches));
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
            average_latency: if total_switches == 0 {
                "—".to_string()
            } else {
                format!("{}ms", total_latency_ms / total_switches as i64)
            },
            static_percentage: if static_switches + dynamic_switches == 0 {
                0
            } else {
                (static_switches * 100 / (static_switches + dynamic_switches))
            },
            today_switch_count: today.total_switches,
            podium,
            muscle_memo_grade: muscle_memo(all, today)
                .map(|result| result.0)
                .unwrap_or_else(|| "—".to_string()),
            muscle_memo_persona: muscle_memo(all, today)
                .map(|result| result.1)
                .unwrap_or_else(|| "Measuring...".to_string()),
            alt_tab_relapse_pct: relapse_percentage(all, today),
            alt_tab_sober_streak: sober_streak(all, today),
            wasted_keystrokes: wasted_keystrokes(all, today),
            personal_best: personal_best(all, today),
        }
    }
}

fn muscle_memo(all: &[DailyBucket], today: &DailyBucket) -> Option<(String, String)> {
    let static_switches = all
        .iter()
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

fn relapse_percentage(all: &[DailyBucket], today: &DailyBucket) -> i32 {
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

fn wasted_keystrokes(all: &[DailyBucket], today: &DailyBucket) -> i32 {
    all.iter()
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
        if total > 0 && bucket.total_switches * 100 / total <= 95 {
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
    date.strip_prefix("day-")
        .and_then(|value| value.parse().ok())
        .unwrap_or(-1)
}
fn format_duration(milliseconds: i64) -> String {
    if milliseconds <= 0 {
        return "—".to_string();
    }
    let seconds = milliseconds / 1000;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
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
        let metrics = StatsCalculator::compute(&[], &today);
        assert_eq!(metrics.total_switch_count, 1);
        assert_eq!(metrics.total_peek_count, 1);
        assert_eq!(metrics.static_percentage, 100);
        assert_eq!(metrics.podium[0].process_name, "notepad.exe");
    }
}
