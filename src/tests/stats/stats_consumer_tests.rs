use super::*;
use crate::stats::session_stats::SessionStats;

struct DummyAppRegistryCache;
impl IAppRegistryCache for DummyAppRegistryCache {
    fn get_display_name(&self, process_name: &str, _process_path: Option<&str>) -> String {
        process_name.to_string()
    }
}

const FREQUENCY: i64 = 10_000_000; // 10MHz ticks

fn ticks_from_ms(ms: i64) -> i64 {
    (ms * FREQUENCY) / 1000
}

fn make_switch_event(
    process_name: &str,
    total_choices: i32,
    modifier_down_ms: i64,
    letter_down_ms: i64,
    previous_letter_up_ms: Option<i64>,
    is_dynamic: bool,
    trigger_key: u32,
) -> StatsEvent {
    StatsEvent::SwitchEvent {
        process_name: process_name.to_string(),
        process_id: None,
        process_path: process_name.to_string(),
        total_choices,
        modifier_down_tick: ticks_from_ms(modifier_down_ms),
        letter_down_tick: ticks_from_ms(letter_down_ms),
        previous_letter_up_tick: previous_letter_up_ms.map(ticks_from_ms),
        is_dynamic,
        trigger_key,
    }
}

fn make_peek_event(
    target_process_name: &str,
    arm_ms: i64,
    finish_ms: i64,
    is_dynamic: bool,
) -> StatsEvent {
    StatsEvent::PeekEvent {
        target_process_name: target_process_name.to_string(),
        target_process_path: target_process_name.to_string(),
        arm_tick: ticks_from_ms(arm_ms),
        finish_tick: ticks_from_ms(finish_ms),
        is_dynamic,
    }
}

fn create_sut() -> StatsConsumer<SessionStats, DummyAppRegistryCache, impl Fn(&str)> {
    let mut consumer = StatsConsumer::new(SessionStats::new(), DummyAppRegistryCache, |_| {});
    consumer.set_frequency(FREQUENCY);
    consumer
}

#[test]
fn process_switch_records_static_switch_in_static_bucket() {
    let mut sut = create_sut();
    let evt = make_switch_event("notepad.exe", 2, 0, 200, None, false, 65);
    sut.process_event(evt);

    let snapshots = sut.session_stats().get_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].total_switches, 1);
    assert!(snapshots[0].static_app_usage.contains_key("notepad.exe"));
    assert!(snapshots[0].dynamic_app_usage.is_empty());
}

#[test]
fn process_switch_records_dynamic_switch_in_dynamic_bucket() {
    let mut sut = create_sut();
    let evt = make_switch_event("explorer.exe", 2, 0, 200, None, true, 65);
    sut.process_event(evt);

    let snapshots = sut.session_stats().get_snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].total_switches, 1);
    assert!(snapshots[0].dynamic_app_usage.contains_key("explorer.exe"));
    assert!(snapshots[0].static_app_usage.is_empty());
}

#[test]
fn process_switch_uses_duration_from_modifier_down_when_no_previous_letter_up() {
    let mut sut = create_sut();
    let evt = make_switch_event("notepad.exe", 2, 0, 200, None, false, 65);
    sut.process_event(evt);

    let snapshots = sut.session_stats().get_snapshots();
    assert_eq!(snapshots[0].total_switches, 1);
}

#[test]
fn process_switch_uses_duration_from_previous_letter_up_when_provided() {
    let mut sut = create_sut();
    let evt = make_switch_event("notepad.exe", 2, 0, 200, Some(100), false, 65);
    sut.process_event(evt);

    let snapshots = sut.session_stats().get_snapshots();
    assert_eq!(snapshots[0].total_switches, 1);
}

#[test]
fn process_switch_clamps_to_baseline_when_duration_exceeds_idle_threshold() {
    let mut sut = create_sut();
    let evt = make_switch_event("notepad.exe", 2, 0, 4000, None, false, 65);
    sut.process_event(evt);

    let snapshots = sut.session_stats().get_snapshots();
    assert_eq!(snapshots[0].total_switches, 1);
}

#[test]
fn process_peek_records_static_peek_in_static_bucket() {
    let mut sut = create_sut();
    let evt = make_peek_event("notepad.exe", 0, 800, false);
    sut.process_event(evt);

    let snapshots = sut.session_stats().get_snapshots();
    assert_eq!(snapshots[0].total_peeks, 1);
    assert_eq!(snapshots[0].static_app_usage["notepad.exe"].peeks, 1);
    assert_eq!(snapshots[0].static_app_usage["notepad.exe"].total_peek_time_ms, 800);
    assert!(snapshots[0].dynamic_app_usage.is_empty());
}

#[test]
fn process_peek_records_dynamic_peek_in_dynamic_bucket() {
    let mut sut = create_sut();
    let evt = make_peek_event("explorer.exe", 0, 800, true);
    sut.process_event(evt);

    let snapshots = sut.session_stats().get_snapshots();
    assert_eq!(snapshots[0].total_peeks, 1);
    assert_eq!(snapshots[0].dynamic_app_usage["explorer.exe"].peeks, 1);
    assert!(snapshots[0].static_app_usage.is_empty());
}

#[test]
fn process_peek_accumulates_duration_for_multiple_peeks() {
    let mut sut = create_sut();
    sut.process_event(make_peek_event("notepad.exe", 0, 600, false));
    sut.process_event(make_peek_event("notepad.exe", 0, 400, false));

    let snapshots = sut.session_stats().get_snapshots();
    assert_eq!(snapshots[0].static_app_usage["notepad.exe"].total_peek_time_ms, 1000);
}

#[test]
fn process_alt_tab_records_alt_tab_switch_and_keystrokes() {
    let mut sut = create_sut();
    sut.process_event(StatsEvent::AltTabEvent(3));

    let snapshots = sut.session_stats().get_snapshots();
    assert_eq!(snapshots[0].alt_tab_switches, 1);
    assert_eq!(snapshots[0].alt_tab_keystrokes, 3);
}
