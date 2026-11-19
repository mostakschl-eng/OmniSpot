use std::collections::HashMap;
pub(crate) use super::storage::{DailyBucketDocument, FastestSwitchRecord};
#[cfg(test)]
pub(crate) use super::storage::AppUsageStats;
use super::IAppRegistryCache;

pub(crate) fn parse_ymd(s: &str) -> Option<(i32, u32, u32)> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let y = parts[0].parse().ok()?;
    let m = parts[1].parse().ok()?;
    let d = parts[2].parse().ok()?;
    Some((y, m, d))
}

pub(crate) fn ymd_to_days(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u32;
    let m = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * m + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + (doe as i64) - 719468
}

pub(crate) fn days_to_ymd(days: i64) -> String {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

pub(crate) fn add_days(date_str: &str, delta_days: i64) -> String {
    if let Some((y, m, d)) = parse_ymd(date_str) {
        let days = ymd_to_days(y, m, d) + delta_days;
        days_to_ymd(days)
    } else {
        date_str.to_string()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatsMetrics {
    pub life_gained: String,
    pub teleport_streak: i32,
    pub muscle_memo_grade: String,
    pub muscle_memo_persona: String,
    pub podium: Vec<AppUsageSummary>,
    pub most_peeked_app: Option<AppUsageSummary>,
    pub first_stale_shortcut: Option<StaleShortcutInfo>,
    pub is_maintenance_warmup: bool,
    pub is_session_warmup: bool,
    pub alt_tab_relapse_pct: i32,
    pub alt_tab_sober_streak: i32,
    pub wasted_keystrokes: i32,
    pub total_switch_count: i32,
    pub personal_best_record: Option<FastestSwitchRecord>,
    pub total_peek_count: i32,
    pub avg_glance_sec: String,
    pub avg_latency: String,
    pub static_pct: i32,
    pub today_switch_count: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppUsageSummary {
    pub process_name: String,
    pub display_name: String,
    pub switches: i32,
    pub peeks: i32,
    pub total_peek_time_ms: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StaleShortcutInfo {
    pub letter: String,
    pub process_name: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppAggregateStats {
    pub switches: i32,
    pub peeks: i32,
    pub total_peek_time_ms: i32,
    pub total_switch_time_ms: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MuscleMemoResult {
    pub grade: String,
    pub persona: String,
}

pub(crate) struct StatsCalculator<T: IAppRegistryCache> {
    app_registry_cache: T,
}

impl<T: IAppRegistryCache> StatsCalculator<T> {
    pub(crate) fn new(app_registry_cache: T) -> Self {
        Self { app_registry_cache }
    }

    pub(crate) fn compute(
        &self,
        all_buckets: &[DailyBucketDocument],
        today: &DailyBucketDocument,
        configured_static_apps: &[(String, String)],
    ) -> StatsMetrics {
        let cutoff = add_days(&today.date, -30);
        let recent_buckets: Vec<&DailyBucketDocument> = all_buckets
            .iter()
            .filter(|b| b.date >= cutoff)
            .collect();

        let combined = Self::build_combined_app_stats(&recent_buckets, today);
        let is_maintenance_warmup = all_buckets.len() < 14;

        let total_switch_count: i32 = recent_buckets.iter().map(|b| b.total_switches).sum::<i32>()
            + today.total_switches;
        let is_session_warmup = total_switch_count < 10;

        let muscle_memo = if is_session_warmup {
            None
        } else {
            Self::compute_muscle_memo_grade(&recent_buckets, today)
        }
        .unwrap_or_else(|| MuscleMemoResult {
            grade: "—".to_string(),
            persona: "Measuring...".to_string(),
        });

        StatsMetrics {
            life_gained: Self::compute_life_gained(all_buckets, today),
            teleport_streak: Self::compute_streak(all_buckets, &today.date, |b| b.total_switches >= 20),
            muscle_memo_grade: muscle_memo.grade,
            muscle_memo_persona: muscle_memo.persona,
            podium: Self::build_podium(&combined, &self.app_registry_cache),
            most_peeked_app: Self::find_most_peeked(&combined, &self.app_registry_cache),
            first_stale_shortcut: if is_maintenance_warmup {
                None
            } else {
                Self::find_first_stale_shortcut(
                    &recent_buckets,
                    today,
                    configured_static_apps,
                    &self.app_registry_cache,
                )
            },
            is_maintenance_warmup,
            is_session_warmup,
            alt_tab_relapse_pct: Self::compute_relapse_pct(&recent_buckets, today),
            alt_tab_sober_streak: Self::compute_streak(all_buckets, &today.date, Self::is_sober_day),
            wasted_keystrokes: Self::compute_wasted_keystrokes(&recent_buckets, today),
            total_switch_count,
            personal_best_record: if is_session_warmup {
                None
            } else {
                Self::find_personal_best(all_buckets, today)
            },
            total_peek_count: recent_buckets.iter().map(|b| b.total_peeks).sum::<i32>()
                + today.total_peeks,
            avg_glance_sec: Self::compute_avg_glance(&combined),
            avg_latency: if is_session_warmup {
                "—".to_string()
            } else {
                Self::compute_avg_latency(&combined)
            },
            static_pct: Self::compute_static_pct(&recent_buckets, today),
            today_switch_count: today.total_switches,
        }
    }

    pub(crate) fn compute_life_gained(
        all_buckets: &[DailyBucketDocument],
        today: &DailyBucketDocument,
    ) -> String {
        let total_ms: i64 = all_buckets
            .iter()
            .map(|b| b.total_time_saved_ms as i64)
            .sum::<i64>()
            + (today.total_time_saved_ms as i64);

        if total_ms <= 0 {
            return "—".to_string();
        }

        let total_seconds = total_ms / 1000;
        if total_seconds < 60 {
            return format!("{}s", total_seconds);
        }

        let total_minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        if total_minutes < 60 {
            return if seconds > 0 {
                format!("{}m {}s", total_minutes, seconds)
            } else {
                format!("{}m", total_minutes)
            };
        }

        let total_hours = total_minutes / 60;
        let minutes = total_minutes % 60;
        format!("{}h {}m", total_hours, minutes)
    }

    pub(crate) fn compute_muscle_memo_grade(
        recent: &[&DailyBucketDocument],
        today: &DailyBucketDocument,
    ) -> Option<MuscleMemoResult> {
        let static_switches: i32 = recent
            .iter()
            .map(|b| b.static_app_usage.values().map(|v| v.switches).sum::<i32>())
            .sum::<i32>()
            + today.static_app_usage.values().map(|v| v.switches).sum::<i32>();

        let dynamic_switches: i32 = recent
            .iter()
            .map(|b| b.dynamic_app_usage.values().map(|v| v.switches).sum::<i32>())
            .sum::<i32>()
            + today.dynamic_app_usage.values().map(|v| v.switches).sum::<i32>();

        let relapses: i32 = recent.iter().map(|b| b.alt_tab_switches).sum::<i32>()
            + today.alt_tab_switches;

        let total = static_switches + dynamic_switches + relapses;
        if total == 0 {
            return None;
        }

        let index = (1.0 * (static_switches as f64) + 0.7 * (dynamic_switches as f64))
            / (total as f64)
            * 100.0;

        let (grade, persona) = if index >= 96.0 {
            ("S", "Shadow Walker")
        } else if index >= 85.0 {
            ("A", "Teleporter")
        } else if index >= 70.0 {
            ("B", "The Navigator")
        } else if index >= 50.0 {
            ("C", "Learner")
        } else if index >= 30.0 {
            ("D", "Novice")
        } else {
            ("F", "Alt-Tabber")
        };

        Some(MuscleMemoResult {
            grade: grade.to_string(),
            persona: persona.to_string(),
        })
    }

    pub(crate) fn compute_streak<F>(
        all_buckets: &[DailyBucketDocument],
        today: &str,
        predicate: F,
    ) -> i32
    where
        F: Fn(&DailyBucketDocument) -> bool,
    {
        let mut streak = 0;
        let mut check_date = add_days(today, -1);

        for bucket in all_buckets {
            if bucket.date != check_date || !predicate(bucket) {
                break;
            }
            streak += 1;
            check_date = add_days(&check_date, -1);
        }

        streak
    }

    pub(crate) fn compute_relapse_pct(
        recent: &[&DailyBucketDocument],
        today: &DailyBucketDocument,
    ) -> i32 {
        let alt_tab_total: i32 = recent.iter().map(|b| b.alt_tab_switches).sum::<i32>()
            + today.alt_tab_switches;
        let switch_total: i32 = recent.iter().map(|b| b.total_switches).sum::<i32>()
            + today.total_switches;
        let grand_total = alt_tab_total + switch_total;

        if grand_total == 0 {
            0
        } else {
            ((alt_tab_total as f64 * 100.0 / grand_total as f64).round()) as i32
        }
    }

    pub(crate) fn is_sober_day(bucket: &DailyBucketDocument) -> bool {
        let total = bucket.total_switches + bucket.alt_tab_switches;
        total == 0 || (bucket.total_switches as f64 * 100.0 / total as f64) > 95.0
    }

    pub(crate) fn compute_wasted_keystrokes(
        recent: &[&DailyBucketDocument],
        today: &DailyBucketDocument,
    ) -> i32 {
        let recent_wasted: i32 = recent
            .iter()
            .map(|b| (b.alt_tab_keystrokes - b.alt_tab_switches).max(0))
            .sum();
        let today_wasted = (today.alt_tab_keystrokes - today.alt_tab_switches).max(0);
        recent_wasted + today_wasted
    }

    pub(crate) fn compute_static_pct(
        recent: &[&DailyBucketDocument],
        today: &DailyBucketDocument,
    ) -> i32 {
        let static_total: i32 = recent
            .iter()
            .map(|b| b.static_app_usage.values().map(|v| v.switches).sum::<i32>())
            .sum::<i32>()
            + today.static_app_usage.values().map(|v| v.switches).sum::<i32>();

        let dynamic_total: i32 = recent
            .iter()
            .map(|b| b.dynamic_app_usage.values().map(|v| v.switches).sum::<i32>())
            .sum::<i32>()
            + today.dynamic_app_usage.values().map(|v| v.switches).sum::<i32>();

        let grand_total = static_total + dynamic_total;
        if grand_total == 0 {
            0
        } else {
            ((static_total as f64 * 100.0 / grand_total as f64).round()) as i32
        }
    }

    pub(crate) fn find_personal_best(
        all_buckets: &[DailyBucketDocument],
        today: &DailyBucketDocument,
    ) -> Option<FastestSwitchRecord> {
        let mut candidates: Vec<&FastestSwitchRecord> = all_buckets
            .iter()
            .filter_map(|b| b.fastest_switch.as_ref())
            .collect();

        if let Some(today_fastest) = &today.fastest_switch {
            candidates.push(today_fastest);
        }

        candidates.into_iter().min_by_key(|f| f.duration_ms).cloned()
    }

    pub(crate) fn compute_avg_glance(combined: &HashMap<String, AppAggregateStats>) -> String {
        let total_peek_ms: i64 = combined.values().map(|v| v.total_peek_time_ms as i64).sum();
        let total_peeks: i32 = combined.values().map(|v| v.peeks).sum();

        if total_peeks == 0 {
            "—".to_string()
        } else {
            format!("{:.1}s", (total_peek_ms as f64) / ((total_peeks as f64) * 1000.0))
        }
    }

    pub(crate) fn compute_avg_latency(combined: &HashMap<String, AppAggregateStats>) -> String {
        let total_switch_time_ms: i64 = combined
            .values()
            .map(|v| v.total_switch_time_ms as i64)
            .sum();
        let total_switches: i32 = combined.values().map(|v| v.switches).sum();

        if total_switches == 0 || total_switch_time_ms == 0 {
            "—".to_string()
        } else {
            format!("{}ms", total_switch_time_ms / (total_switches as i64))
        }
    }

    pub(crate) fn build_podium(
        combined: &HashMap<String, AppAggregateStats>,
        app_registry_cache: &T,
    ) -> Vec<AppUsageSummary> {
        let mut entries: Vec<(&String, &AppAggregateStats)> = combined.iter().collect();
        entries.sort_by(|a, b| b.1.switches.cmp(&a.1.switches));

        entries
            .into_iter()
            .take(3)
            .map(|(key, val)| AppUsageSummary {
                process_name: key.clone(),
                display_name: app_registry_cache.get_display_name(key, None),
                switches: val.switches,
                peeks: val.peeks,
                total_peek_time_ms: val.total_peek_time_ms,
            })
            .collect()
    }

    pub(crate) fn find_most_peeked(
        combined: &HashMap<String, AppAggregateStats>,
        app_registry_cache: &T,
    ) -> Option<AppUsageSummary> {
        let mut with_peeks: Vec<(&String, &AppAggregateStats)> = combined
            .iter()
            .filter(|(_, val)| val.peeks > 0)
            .collect();

        with_peeks.sort_by(|a, b| b.1.peeks.cmp(&a.1.peeks));

        with_peeks.first().map(|(key, val)| AppUsageSummary {
            process_name: (*key).clone(),
            display_name: app_registry_cache.get_display_name(key, None),
            switches: val.switches,
            peeks: val.peeks,
            total_peek_time_ms: val.total_peek_time_ms,
        })
    }

    pub(crate) fn find_first_stale_shortcut(
        recent: &[&DailyBucketDocument],
        today: &DailyBucketDocument,
        configured_static_apps: &[(String, String)],
        app_registry_cache: &T,
    ) -> Option<StaleShortcutInfo> {
        for (process_name, letter) in configured_static_apps {
            let recent_switches: i32 = recent
                .iter()
                .map(|b| {
                    b.static_app_usage
                        .get(process_name)
                        .map(|s| s.switches)
                        .unwrap_or(0)
                })
                .sum();

            let today_switches = today
                .static_app_usage
                .get(process_name)
                .map(|s| s.switches)
                .unwrap_or(0);

            if recent_switches + today_switches == 0 {
                return Some(StaleShortcutInfo {
                    letter: letter.clone(),
                    process_name: process_name.clone(),
                    display_name: app_registry_cache.get_display_name(process_name, None),
                });
            }
        }

        None
    }

    pub(crate) fn build_combined_app_stats(
        recent: &[&DailyBucketDocument],
        today: &DailyBucketDocument,
    ) -> HashMap<String, AppAggregateStats> {
        let mut combined = HashMap::new();

        for bucket in recent.iter().copied().chain(std::iter::once(today)) {
            for (key, stats) in &bucket.static_app_usage {
                let entry = combined
                    .entry(key.to_ascii_lowercase())
                    .or_insert_with(AppAggregateStats::default);
                entry.switches += stats.switches;
                entry.peeks += stats.peeks;
                entry.total_peek_time_ms += stats.total_peek_time_ms;
                entry.total_switch_time_ms += stats.total_switch_time_ms;
            }
            for (key, stats) in &bucket.dynamic_app_usage {
                let entry = combined
                    .entry(key.to_ascii_lowercase())
                    .or_insert_with(AppAggregateStats::default);
                entry.switches += stats.switches;
                entry.peeks += stats.peeks;
                entry.total_peek_time_ms += stats.total_peek_time_ms;
                entry.total_switch_time_ms += stats.total_switch_time_ms;
            }
        }

        combined
    }
}

#[cfg(test)]
#[path = "../tests/Stats/StatsCalculatorTests.rs"]
mod tests;
