use super::*;

#[test]
fn cache_reuses_unexpired_snapshot_and_refreshes_after_expiry() {
    let sentinel = ApplicationWindow {
        handle: i64::MAX,
        ..Default::default()
    };
    let mut enumerator = WindowEnumerator {
        cache: Some((
            SystemTime::now() + Duration::from_secs(10),
            vec![sentinel.clone()],
        )),
    };
    assert_eq!(enumerator.get_cached_windows()[0].handle, i64::MAX);
    enumerator.cache = Some((SystemTime::now() - Duration::from_secs(1), vec![sentinel]));
    assert!(enumerator
        .get_cached_windows()
        .iter()
        .all(|window| window.handle != i64::MAX));
    assert!(WindowEnumerator::default().cache.is_none());
}
