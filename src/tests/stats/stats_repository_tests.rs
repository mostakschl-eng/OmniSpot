use std::sync::{Arc, Mutex};
use crate::stats::stats_repository::StatsRepository;
use crate::stats::storage::{DailyBucketDocument, StatsDatabase};

#[test]
fn get_all_historic_buckets_returns_only_past_buckets_sorted_descending() {
    let mut db = StatsDatabase::default();

    let doc_old = DailyBucketDocument {
        date: "2026-11-01".to_string(),
        total_switches: 10,
        ..Default::default()
    };
    let doc_yesterday = DailyBucketDocument {
        date: "2026-11-17".to_string(),
        total_switches: 25,
        ..Default::default()
    };
    let doc_today = DailyBucketDocument {
        date: "2026-11-18".to_string(),
        total_switches: 50,
        ..Default::default()
    };
    let doc_tomorrow = DailyBucketDocument {
        date: "2026-11-19".to_string(),
        total_switches: 5,
        ..Default::default()
    };

    db.daily_buckets.insert("2026-11-01".to_string(), serde_json::to_value(&doc_old).unwrap());
    db.daily_buckets.insert("2026-11-17".to_string(), serde_json::to_value(&doc_yesterday).unwrap());
    db.daily_buckets.insert("2026-11-18".to_string(), serde_json::to_value(&doc_today).unwrap());
    db.daily_buckets.insert("2026-11-19".to_string(), serde_json::to_value(&doc_tomorrow).unwrap());

    let sut = StatsRepository::new(Arc::new(Mutex::new(db)));
    let results = sut.get_all_historic_buckets(Some("2026-11-18"));

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].date, "2026-11-17");
    assert_eq!(results[0].total_switches, 25);
    assert_eq!(results[1].date, "2026-11-01");
    assert_eq!(results[1].total_switches, 10);
}

#[test]
fn get_all_historic_buckets_returns_empty_when_no_past_buckets() {
    let mut db = StatsDatabase::default();
    let doc_today = DailyBucketDocument {
        date: "2026-11-18".to_string(),
        total_switches: 50,
        ..Default::default()
    };
    db.daily_buckets.insert("2026-11-18".to_string(), serde_json::to_value(&doc_today).unwrap());

    let sut = StatsRepository::new(Arc::new(Mutex::new(db)));
    let results = sut.get_all_historic_buckets(Some("2026-11-18"));

    assert!(results.is_empty());
}
