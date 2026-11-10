use super::*;

#[test]
fn spotlight_launch_classification_preserves_protocol_executable_and_package_rules() {
    assert!(!is_packaged_search_target("MS-SETTINGS:display", "App"));
    assert!(!is_packaged_search_target("App.EXE", "App"));
    assert!(!is_packaged_search_target(r"C:\Folder", "Folder"));
    assert!(is_packaged_search_target("Family!App", "App"));
    let path = std::env::temp_dir().join(format!("omnispot-launch-{}.lnk", std::process::id()));
    std::fs::write(&path, b"test").unwrap();
    assert!(!is_packaged_search_target(path.to_str().unwrap(), "App"));
    std::fs::remove_file(path).unwrap();
}
