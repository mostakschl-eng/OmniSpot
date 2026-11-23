use super::*;
use std::collections::BTreeMap;

#[test]
fn snapshot_returns_zeroes_when_no_events_recorded() {
    let sut = SessionStats::new();
    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots.len(), 1);

    let result = &snapshots[0];
    assert_eq!(result.total_switches, 0);
    assert_eq!(result.total_time_saved_ms, 0);
    assert_eq!(result.total_peeks, 0);
    assert!(result.static_app_usage.is_empty());
    assert!(result.dynamic_app_usage.is_empty());
}

#[test]
fn record_switch_increments_total_switches() {
    let sut = SessionStats::new();
    sut.record_switch("notepad.exe", None, 100, 100, false, None, "A");
    sut.record_switch("notepad.exe", None, 100, 200, false, None, "A");

    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].total_switches, 2);
}

#[test]
fn record_switch_accumulates_total_time_saved_ms() {
    let sut = SessionStats::new();
    sut.record_switch("notepad.exe", None, 100, 300, false, None, "A");
    sut.record_switch("code.exe", None, 100, 500, false, None, "A");

    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].total_time_saved_ms, 800);
}

#[test]
fn record_switch_accumulates_total_switch_time_ms() {
    let sut = SessionStats::new();
    sut.record_switch("notepad.exe", None, 200, 100, false, None, "A");
    sut.record_switch("notepad.exe", None, 300, 100, false, None, "A");

    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        snapshots[0].static_app_usage["notepad.exe"].total_switch_time_ms,
        500
    );
}

#[test]
fn record_switch_tracks_static_app_switch_count() {
    let sut = SessionStats::new();
    sut.record_switch("notepad.exe", None, 100, 100, false, None, "A");
    sut.record_switch("notepad.exe", None, 100, 100, false, None, "A");
    sut.record_switch("code.exe", None, 100, 100, false, None, "A");

    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots[0].static_app_usage["notepad.exe"].switches, 2);
    assert_eq!(snapshots[0].static_app_usage["code.exe"].switches, 1);
    assert!(snapshots[0].dynamic_app_usage.is_empty());
}

#[test]
fn record_switch_tracks_dynamic_app_switch_count() {
    let sut = SessionStats::new();
    sut.record_switch("explorer.exe", None, 100, 100, true, None, "A");
    sut.record_switch("explorer.exe", None, 100, 100, true, None, "A");

    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots[0].dynamic_app_usage["explorer.exe"].switches, 2);
    assert!(snapshots[0].static_app_usage.is_empty());
}

#[test]
fn record_switch_keeps_static_and_dynamic_buckets_separate() {
    let sut = SessionStats::new();
    sut.record_switch("notepad.exe", None, 100, 100, false, None, "A");
    sut.record_switch("explorer.exe", None, 100, 100, true, None, "A");

    let snapshots = sut.get_snapshots();
    assert!(snapshots[0].static_app_usage.contains_key("notepad.exe"));
    assert!(!snapshots[0].static_app_usage.contains_key("explorer.exe"));
    assert!(snapshots[0].dynamic_app_usage.contains_key("explorer.exe"));
    assert!(!snapshots[0].dynamic_app_usage.contains_key("notepad.exe"));
}

#[test]
fn record_peek_increments_total_peeks() {
    let sut = SessionStats::new();
    sut.record_peek("notepad.exe", 600, false);
    sut.record_peek("notepad.exe", 400, false);

    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots[0].total_peeks, 2);
}

#[test]
fn record_peek_accumulates_peek_time_in_static_bucket() {
    let sut = SessionStats::new();
    sut.record_peek("notepad.exe", 600, false);
    sut.record_peek("notepad.exe", 400, false);

    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots[0].static_app_usage["notepad.exe"].peeks, 2);
    assert_eq!(
        snapshots[0].static_app_usage["notepad.exe"].total_peek_time_ms,
        1000
    );
    assert!(snapshots[0].dynamic_app_usage.is_empty());
}

#[test]
fn record_peek_accumulates_peek_time_in_dynamic_bucket() {
    let sut = SessionStats::new();
    sut.record_peek("explorer.exe", 500, true);

    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots[0].dynamic_app_usage["explorer.exe"].peeks, 1);
    assert_eq!(
        snapshots[0].dynamic_app_usage["explorer.exe"].total_peek_time_ms,
        500
    );
    assert!(snapshots[0].static_app_usage.is_empty());
}

#[test]
fn load_from_restores_all_counters() {
    let sut = SessionStats::new();
    let today = system_today();

    let mut static_map = BTreeMap::new();
    static_map.insert("code.exe".to_string(), AppUsageStats::new(7, 2, 1200, 0));
    let mut dynamic_map = BTreeMap::new();
    dynamic_map.insert("explorer.exe".to_string(), AppUsageStats::new(3, 0, 0, 0));

    let doc = DailyBucketDocument {
        date: today,
        total_switches: 10,
        total_time_saved_ms: 5000,
        total_peeks: 3,
        static_app_usage: static_map,
        dynamic_app_usage: dynamic_map,
        ..Default::default()
    };

    sut.load_from(&doc);
    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots.len(), 1);

    let res = &snapshots[0];
    assert_eq!(res.total_switches, 10);
    assert_eq!(res.total_time_saved_ms, 5000);
    assert_eq!(res.total_peeks, 3);
    assert_eq!(res.static_app_usage["code.exe"].switches, 7);
    assert_eq!(res.dynamic_app_usage["explorer.exe"].switches, 3);
}

#[test]
fn record_alt_tab_increments_and_accumulates() {
    let sut = SessionStats::new();
    sut.record_alt_tab(1);
    sut.record_alt_tab(3);

    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots[0].alt_tab_switches, 2);
    assert_eq!(snapshots[0].alt_tab_keystrokes, 4);
}

#[test]
fn record_switch_sets_and_updates_fastest_switch() {
    let sut = SessionStats::new();
    sut.record_switch("notepad.exe", None, 100, 100, false, Some(400), "N");
    sut.record_switch("spotify.exe", None, 100, 100, false, Some(150), "S");

    let snapshots = sut.get_snapshots();
    let fastest = snapshots[0].fastest_switch.as_ref().unwrap();
    assert_eq!(fastest.duration_ms, 150);
    assert_eq!(fastest.app_name, "spotify.exe");
    assert_eq!(fastest.letter, "S");
}

#[test]
fn record_switch_triggers_rollover_when_date_changed() {
    let sut = SessionStats::new();
    let today = system_today();
    let yesterday = "2026-11-17".to_string();

    sut.set_today_override(Some(today.clone()));
    sut.record_switch("notepad.exe", None, 100, 100, false, None, "A");
    sut.simulate_last_recorded_date(yesterday.clone());

    sut.record_switch("code.exe", None, 100, 100, false, None, "A");

    let snapshots = sut.get_snapshots();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].date, yesterday);
    assert_eq!(snapshots[0].total_switches, 1);
    assert_eq!(snapshots[1].date, today);
    assert_eq!(snapshots[1].total_switches, 1);
}

#[test]
fn clear_removes_rollover_snapshot() {
    let sut = SessionStats::new();
    let today = system_today();
    let yesterday = "2026-11-17".to_string();

    sut.set_today_override(Some(today));
    sut.record_switch("notepad.exe", None, 100, 100, false, None, "A");
    sut.simulate_last_recorded_date(yesterday);
    sut.record_switch("code.exe", None, 100, 100, false, None, "A");
    assert_eq!(sut.get_snapshots().len(), 2);

    sut.clear();
    assert_eq!(sut.get_snapshots().len(), 1);
}

#[test]
fn repeated_rollovers_keep_unflushed_days_and_resume_a_pending_day() {
    let stats = SessionStats::new();
    for date in ["2026-11-20", "2026-11-21", "2026-11-22", "2026-11-20"] {
        stats.set_today_override(Some(date.into()));
        stats.record_alt_tab(1);
    }
    let snapshots = stats.get_snapshots();
    assert_eq!(snapshots.len(), 3);
    assert_eq!(snapshots.last().unwrap().date, "2026-11-20");
    assert_eq!(snapshots.last().unwrap().alt_tab_switches, 2);
    assert!(snapshots.iter().any(|day| day.date == "2026-11-21"));
    assert!(snapshots.iter().any(|day| day.date == "2026-11-22"));

    stats.clear_rollover();
    assert_eq!(stats.get_snapshots().len(), 1);
}
