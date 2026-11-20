use super::*;
use serde_json::json;

#[test]
fn up_removes_default_date_bucket_and_keeps_others() {
    let mut db = StatsDatabase::default();
    db.daily_buckets.insert("0001-01-01".to_string(), json!({ "TotalSwitches": 0 }));
    db.daily_buckets.insert("2026-04-28".to_string(), json!({ "TotalSwitches": 30 }));

    let sut = RemoveDefaultDateBucketMigration;
    let result = sut.up(&mut db);
    assert!(result.is_ok());

    assert_eq!(db.daily_buckets.len(), 1);
    assert!(db.daily_buckets.contains_key("2026-04-28"));
    assert_eq!(db.daily_buckets["2026-04-28"]["TotalSwitches"], 30);
}

#[test]
fn up_does_nothing_when_no_default_date_bucket() {
    let mut db = StatsDatabase::default();
    db.daily_buckets.insert("2026-04-28".to_string(), json!({ "TotalSwitches": 30 }));

    let sut = RemoveDefaultDateBucketMigration;
    let result = sut.up(&mut db);
    assert!(result.is_ok());

    assert_eq!(db.daily_buckets.len(), 1);
}
