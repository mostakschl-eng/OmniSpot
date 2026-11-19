use super::*;

#[test]
fn get_link_path_combines_startup_dir_and_app_name() {
    let temp_dir = std::env::temp_dir().join("omnispot_test_startup");
    let sut = AutoStart::with_startup_dir("OmniSpot", temp_dir.clone());

    let link_path = sut.get_link_path().unwrap();
    assert_eq!(link_path, temp_dir.join("OmniSpot.lnk"));
}

#[test]
fn is_enabled_returns_true_when_file_exists() {
    let temp_dir = std::env::temp_dir().join("omnispot_test_startup_enabled");
    let _ = std::fs::create_dir_all(&temp_dir);
    let lnk_path = temp_dir.join("OmniSpot.lnk");
    std::fs::write(&lnk_path, b"dummy shortcut").unwrap();

    let sut = AutoStart::with_startup_dir("OmniSpot", temp_dir.clone());
    assert!(sut.is_enabled());

    let _ = std::fs::remove_file(lnk_path);
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn is_enabled_returns_false_when_file_does_not_exist() {
    let temp_dir = std::env::temp_dir().join("omnispot_test_startup_disabled");
    let sut = AutoStart::with_startup_dir("OmniSpot", temp_dir);
    assert!(!sut.is_enabled());
}

#[test]
fn remove_shortcut_deletes_file() {
    let temp_dir = std::env::temp_dir().join("omnispot_test_startup_remove");
    let _ = std::fs::create_dir_all(&temp_dir);
    let lnk_path = temp_dir.join("OmniSpot.lnk");
    std::fs::write(&lnk_path, b"dummy shortcut").unwrap();

    let sut = AutoStart::with_startup_dir("OmniSpot", temp_dir.clone());
    assert!(sut.remove_shortcut());
    assert!(!lnk_path.exists());

    let _ = std::fs::remove_dir_all(temp_dir);
}
