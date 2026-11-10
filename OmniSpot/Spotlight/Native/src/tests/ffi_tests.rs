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
