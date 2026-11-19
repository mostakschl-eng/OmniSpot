use super::*;
use std::collections::BTreeMap;

struct FakeRegistryCache {
    display_names: HashMap<String, String>,
}

impl FakeRegistryCache {
    fn new(map: HashMap<String, String>) -> Self {
        Self { display_names: map }
    }
}

impl IAppRegistryCache for FakeRegistryCache {
    fn get_display_name(&self, process_name: &str, _process_path: Option<&str>) -> String {
        self.display_names
            .get(&process_name.to_ascii_lowercase())
            .cloned()
            .unwrap_or_else(|| process_name.to_string())
    }
}

const TODAY: &str = "2026-11-18";

fn empty_today() -> DailyBucketDocument {
    DailyBucketDocument::new(TODAY)
}

fn bucket_on(date: &str, total_switches: i32, alt_tab_switches: i32) -> DailyBucketDocument {
    DailyBucketDocument {
        date: date.to_string(),
        total_switches,
        alt_tab_switches,
        ..Default::default()
    }
}

// ── ComputeLifeGained ─────────────────────────────────────────────────────

#[test]
fn compute_life_gained_returns_dash_when_no_data() {
    let result = StatsCalculator::<FakeRegistryCache>::compute_life_gained(&[], &empty_today());
    assert_eq!(result, "—");
}

#[test]
fn compute_life_gained_formats_correctly() {
    let cases = [
        (30_000, "30s"),
        (59_000, "59s"),
        (90_000, "1m 30s"),
        (1_800_000, "30m"),
        (3_599_000, "59m 59s"),
        (5_400_000, "1h 30m"),
    ];

    for (time_saved_ms, expected) in cases {
        let today = DailyBucketDocument {
            date: TODAY.to_string(),
            total_time_saved_ms: time_saved_ms,
            ..Default::default()
        };
        let result = StatsCalculator::<FakeRegistryCache>::compute_life_gained(&[], &today);
        assert_eq!(result, expected);
    }
}

#[test]
fn compute_life_gained_accumulates_across_all_buckets() {
    let buckets = [
        DailyBucketDocument {
            date: add_days(TODAY, -1),
            total_time_saved_ms: 3_600_000,
            ..Default::default()
        },
        DailyBucketDocument {
            date: add_days(TODAY, -2),
            total_time_saved_ms: 3_600_000,
            ..Default::default()
        },
    ];
    let today = DailyBucketDocument {
        date: TODAY.to_string(),
        total_time_saved_ms: 3_600_000,
        ..Default::default()
    };

    let result = StatsCalculator::<FakeRegistryCache>::compute_life_gained(&buckets, &today);
    assert_eq!(result, "3h 0m");
}

// ── ComputeMuscleMemoGrade ────────────────────────────────────────────────

#[test]
fn compute_muscle_memo_grade_returns_dash_when_no_data() {
    let result = StatsCalculator::<FakeRegistryCache>::compute_muscle_memo_grade(&[], &empty_today());
    assert!(result.is_none());
}

#[test]
fn compute_muscle_memo_grade_returns_correct_grade() {
    let cases = [
        (96, 0, 4, "S", "Shadow Walker"),
        (85, 0, 15, "A", "Teleporter"),
        (70, 0, 30, "B", "The Navigator"),
        (50, 0, 50, "C", "Learner"),
        (30, 0, 70, "D", "Novice"),
        (0, 0, 100, "F", "Alt-Tabber"),
    ];

    for (static_switches, dynamic_switches, relapses, expected_grade, expected_persona) in cases {
        let mut static_map = BTreeMap::new();
        if static_switches > 0 {
            static_map.insert("app.exe".to_string(), AppUsageStats::new_switch(0));
            static_map.get_mut("app.exe").unwrap().switches = static_switches;
        }
        let mut dynamic_map = BTreeMap::new();
        if dynamic_switches > 0 {
            dynamic_map.insert("app.exe".to_string(), AppUsageStats::new_switch(0));
            dynamic_map.get_mut("app.exe").unwrap().switches = dynamic_switches;
        }

        let today = DailyBucketDocument {
            date: TODAY.to_string(),
            alt_tab_switches: relapses,
            static_app_usage: static_map,
            dynamic_app_usage: dynamic_map,
            ..Default::default()
        };

        let result = StatsCalculator::<FakeRegistryCache>::compute_muscle_memo_grade(&[], &today).unwrap();
        assert_eq!(result.grade, expected_grade);
        assert_eq!(result.persona, expected_persona);
    }
}

// ── ComputeStreak ─────────────────────────────────────────────────────────

#[test]
fn compute_streak_returns_zero_when_no_buckets() {
    let result = StatsCalculator::<FakeRegistryCache>::compute_streak(&[], TODAY, |_| true);
    assert_eq!(result, 0);
}

#[test]
fn compute_streak_returns_zero_when_yesterday_missing() {
    let buckets = [bucket_on(&add_days(TODAY, -2), 25, 0)];
    let result = StatsCalculator::<FakeRegistryCache>::compute_streak(&buckets, TODAY, |b| b.total_switches >= 20);
    assert_eq!(result, 0);
}

#[test]
fn compute_streak_counts_consecutive_days() {
    let buckets = [
        bucket_on(&add_days(TODAY, -1), 25, 0),
        bucket_on(&add_days(TODAY, -2), 25, 0),
        bucket_on(&add_days(TODAY, -3), 25, 0),
    ];
    let result = StatsCalculator::<FakeRegistryCache>::compute_streak(&buckets, TODAY, |b| b.total_switches >= 20);
    assert_eq!(result, 3);
}

#[test]
fn compute_streak_stops_at_gap() {
    let buckets = [
        bucket_on(&add_days(TODAY, -1), 25, 0),
        bucket_on(&add_days(TODAY, -2), 25, 0),
        bucket_on(&add_days(TODAY, -4), 25, 0),
    ];
    let result = StatsCalculator::<FakeRegistryCache>::compute_streak(&buckets, TODAY, |b| b.total_switches >= 20);
    assert_eq!(result, 2);
}

#[test]
fn compute_streak_stops_when_predicate_fails() {
    let buckets = [
        bucket_on(&add_days(TODAY, -1), 25, 0),
        bucket_on(&add_days(TODAY, -2), 5, 0), // below threshold
        bucket_on(&add_days(TODAY, -3), 25, 0),
    ];
    let result = StatsCalculator::<FakeRegistryCache>::compute_streak(&buckets, TODAY, |b| b.total_switches >= 20);
    assert_eq!(result, 1);
}

// ── ComputeRelapsePct ─────────────────────────────────────────────────────

#[test]
fn compute_relapse_pct_returns_zero_when_no_switches() {
    let result = StatsCalculator::<FakeRegistryCache>::compute_relapse_pct(&[], &empty_today());
    assert_eq!(result, 0);
}

#[test]
fn compute_relapse_pct_computes_correct_percentage() {
    let today = DailyBucketDocument {
        date: TODAY.to_string(),
        total_switches: 80,
        alt_tab_switches: 20,
        ..Default::default()
    };
    let result = StatsCalculator::<FakeRegistryCache>::compute_relapse_pct(&[], &today);
    assert_eq!(result, 20);
}

// ── IsSoberDay ────────────────────────────────────────────────────────────

#[test]
fn is_sober_day_returns_true_when_no_activity() {
    assert!(StatsCalculator::<FakeRegistryCache>::is_sober_day(&empty_today()));
}

#[test]
fn is_sober_day_returns_true_when_alt_tab_under_5_pct() {
    let bucket = DailyBucketDocument {
        date: TODAY.to_string(),
        total_switches: 97,
        alt_tab_switches: 3,
        ..Default::default()
    };
    assert!(StatsCalculator::<FakeRegistryCache>::is_sober_day(&bucket));
}

#[test]
fn is_sober_day_returns_false_when_alt_tab_over_5_pct() {
    let bucket = DailyBucketDocument {
        date: TODAY.to_string(),
        total_switches: 90,
        alt_tab_switches: 10,
        ..Default::default()
    };
    assert!(!StatsCalculator::<FakeRegistryCache>::is_sober_day(&bucket));
}

// ── ComputeWastedKeystrokes ───────────────────────────────────────────────

#[test]
fn compute_wasted_keystrokes_counts_extra_keystrokes() {
    let today = DailyBucketDocument {
        date: TODAY.to_string(),
        alt_tab_switches: 5,
        alt_tab_keystrokes: 15,
        ..Default::default()
    };
    let result = StatsCalculator::<FakeRegistryCache>::compute_wasted_keystrokes(&[], &today);
    assert_eq!(result, 10);
}

// ── ComputeStaticPct ──────────────────────────────────────────────────────

#[test]
fn compute_static_pct_computes_correct_ratio() {
    let mut static_map = BTreeMap::new();
    static_map.insert("a.exe".to_string(), AppUsageStats { switches: 75, ..Default::default() });
    let mut dynamic_map = BTreeMap::new();
    dynamic_map.insert("b.exe".to_string(), AppUsageStats { switches: 25, ..Default::default() });

    let today = DailyBucketDocument {
        date: TODAY.to_string(),
        static_app_usage: static_map,
        dynamic_app_usage: dynamic_map,
        ..Default::default()
    };
    let result = StatsCalculator::<FakeRegistryCache>::compute_static_pct(&[], &today);
    assert_eq!(result, 75);
}

// ── FindPersonalBest ──────────────────────────────────────────────────────

#[test]
fn find_personal_best_returns_fastest_across_all_buckets() {
    let buckets = [
        DailyBucketDocument {
            date: add_days(TODAY, -1),
            fastest_switch: Some(FastestSwitchRecord {
                duration_ms: 80,
                app_name: "slow.exe".to_string(),
                letter: "S".to_string(),
                date: add_days(TODAY, -1),
            }),
            ..Default::default()
        },
        DailyBucketDocument {
            date: add_days(TODAY, -2),
            fastest_switch: Some(FastestSwitchRecord {
                duration_ms: 40,
                app_name: "fast.exe".to_string(),
                letter: "F".to_string(),
                date: add_days(TODAY, -2),
            }),
            ..Default::default()
        },
    ];

    let result = StatsCalculator::<FakeRegistryCache>::find_personal_best(&buckets, &empty_today()).unwrap();
    assert_eq!(result.duration_ms, 40);
    assert_eq!(result.app_name, "fast.exe");
}

// ── ComputeAvgLatency & ComputeAvgGlance ───────────────────────────────────

#[test]
fn compute_avg_latency_computes_correct_average() {
    let mut combined = HashMap::new();
    combined.insert(
        "a.exe".to_string(),
        AppAggregateStats {
            switches: 4,
            peeks: 0,
            total_peek_time_ms: 0,
            total_switch_time_ms: 200,
        },
    );

    let result = StatsCalculator::<FakeRegistryCache>::compute_avg_latency(&combined);
    assert_eq!(result, "50ms");
}

#[test]
fn compute_avg_glance_returns_dash_when_no_peeks() {
    let result = StatsCalculator::<FakeRegistryCache>::compute_avg_glance(&HashMap::new());
    assert_eq!(result, "—");
}

// ── BuildPodium ───────────────────────────────────────────────────────────

#[test]
fn build_podium_returns_top_three_by_switch() {
    let mut combined = HashMap::new();
    combined.insert("a.exe".to_string(), AppAggregateStats { switches: 100, ..Default::default() });
    combined.insert("b.exe".to_string(), AppAggregateStats { switches: 50, ..Default::default() });
    combined.insert("c.exe".to_string(), AppAggregateStats { switches: 30, ..Default::default() });
    combined.insert("d.exe".to_string(), AppAggregateStats { switches: 10, ..Default::default() });

    let mut display_names = HashMap::new();
    display_names.insert("a.exe".to_string(), "App A".to_string());
    display_names.insert("b.exe".to_string(), "App B".to_string());
    display_names.insert("c.exe".to_string(), "App C".to_string());
    display_names.insert("d.exe".to_string(), "App D".to_string());

    let cache = FakeRegistryCache::new(display_names);
    let result = StatsCalculator::build_podium(&combined, &cache);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].process_name, "a.exe");
    assert_eq!(result[1].process_name, "b.exe");
    assert_eq!(result[2].process_name, "c.exe");
}

// ── FindFirstStaleShortcut ────────────────────────────────────────────────

#[test]
fn find_first_stale_shortcut_returns_first_unused_app() {
    let configured_apps = vec![
        ("used.exe".to_string(), "U".to_string()),
        ("stale.exe".to_string(), "S".to_string()),
    ];

    let mut static_map = BTreeMap::new();
    static_map.insert("used.exe".to_string(), AppUsageStats { switches: 5, ..Default::default() });

    let today = DailyBucketDocument {
        date: TODAY.to_string(),
        static_app_usage: static_map,
        ..Default::default()
    };

    let mut display_names = HashMap::new();
    display_names.insert("stale.exe".to_string(), "Stale App".to_string());
    let cache = FakeRegistryCache::new(display_names);

    let result = StatsCalculator::find_first_stale_shortcut(&[], &today, &configured_apps, &cache).unwrap();
    assert_eq!(result.letter, "S");
    assert_eq!(result.process_name, "stale.exe");
    assert_eq!(result.display_name, "Stale App");
}
