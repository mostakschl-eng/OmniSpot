use super::*;
#[test]
fn json_response_round_trips_and_frees() {
    let value = vec!["a".repeat(513), "Unicode: 日本語".to_string()];
    let mut length = 0;
    let pointer = write_json_response(&value, &mut length);
    assert!(!pointer.is_null());
    let decoded: Vec<String> =
        serde_json::from_slice(unsafe { slice::from_raw_parts(pointer, length) }).unwrap();
    assert_eq!(decoded, value);
    omnispot_spotlight_free(pointer, length);
}

#[test]
fn rust_stats_queue_drops_oldest_event_when_capacity_is_reached() {
    let state = StatsQueueState {
        queue: (0..1000).map(|nav_count| QueuedStatsEvent::AltTab { nav_count }).collect(),
        running: true,
        stopping: false,
    };
    let runtime = StatsRuntime {
        state: Mutex::new(state),
        wake: Condvar::new(),
        worker: Mutex::new(None),
    };

    assert!(runtime.enqueue(QueuedStatsEvent::AltTab { nav_count: 1000 }));
    let state = runtime.state.lock().unwrap();
    assert_eq!(state.queue.len(), 1000);
    assert!(matches!(state.queue.front(), Some(QueuedStatsEvent::AltTab { nav_count: 1 })));
    assert!(matches!(state.queue.back(), Some(QueuedStatsEvent::AltTab { nav_count: 1000 })));
}

#[test]
fn rust_stats_runtime_applies_enabled_configuration_idempotently() {
    assert_eq!(omnispot_stats_service_update(0), 1);
    assert_eq!(omnispot_stats_service_update(1), 1);
    assert_eq!(omnispot_stats_service_is_running(), 1);
    assert_eq!(omnispot_stats_service_update(1), 1);
    assert_eq!(omnispot_stats_service_update(0), 2);
    assert_eq!(omnispot_stats_service_is_running(), 0);
    assert_eq!(omnispot_stats_service_update(0), 1);
}
