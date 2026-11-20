use super::application_window::ApplicationWindow;
use super::installed_packages::InstalledPackages;
use super::window_enumerator::get_focused_app_windows;
use super::*;
use crate::configuration::configuration::{ApplicationConfiguration, CycleMode};

fn window(handle: i64, path: &str) -> ApplicationWindow {
    ApplicationWindow {
        handle,
        title: Some("Test".into()),
        process_image_path: path.into(),
        width: 100,
        height: 100,
        ..Default::default()
    }
}

#[test]
fn window_validity_matches_existing_size_title_and_style_rules() {
    let valid = window(1, "test.exe");
    assert!(valid.is_valid_window());
    // System.Drawing.Size.IsEmpty only requires both dimensions to be zero.
    assert!(ApplicationWindow {
        width: 0,
        ..valid.clone()
    }
    .is_valid_window());
    assert!(!ApplicationWindow {
        width: 0,
        height: 0,
        ..valid.clone()
    }
    .is_valid_window());
    assert!(!ApplicationWindow {
        title: None,
        ..valid.clone()
    }
    .is_valid_window());
    assert!(!ApplicationWindow {
        title: Some(String::new()),
        ..valid.clone()
    }
    .is_valid_window());
    for style_ex in [0x08000000, 0x00000080, 0x08000080] {
        assert!(!ApplicationWindow {
            style_ex,
            ..valid.clone()
        }
        .is_valid_window());
    }
    assert!(!ApplicationWindow {
        is_cloaked: true,
        ..valid
    }
    .is_valid_window());
}

#[test]
fn packages_match_nested_paths_and_reject_prefix_collisions() {
    let packages = InstalledPackages::new(&[
        r"C:\Apps\Package\".into(),
        r"c:\apps\package".into(),
        r"D:\DevPackage".into(),
    ]);
    assert_eq!(packages.paths.len(), 2);
    for path in [r"c:\APPS\package\bin\app.exe", r"D:\DevPackage\app.exe"] {
        assert!(packages.contains(path));
    }
    for path in [
        r"C:\Apps\PackageOther\app.exe",
        r"C:\AppsOther\Package\app.exe",
        r"C:\Apps\Package",
        r"C:\Apps\app.exe",
        "",
    ] {
        assert!(!packages.contains(path));
    }
    assert!(!InstalledPackages::new(&[]).contains(r"D:\DevPackage\app.exe"));
}

#[test]
fn ordinal_matching_preserves_dotnet_unicode_and_ascii_boundaries() {
    assert!(equal_ignore_case("µ", "Μ"));
    assert!(equal_ignore_case("ς", "Σ"));
    assert!(equal_ignore_case("𐐨", "𐐀"));
    assert!(!equal_ignore_case("ſ", "S"));
    assert!(!equal_ignore_case("ı", "I"));
    let packages = InstalledPackages::new(&[r"C:\Apps\µ".into()]);
    assert!(packages.contains(r"C:\APPS\Μ\bin\app.exe"));
}

#[test]
fn focused_windows_keep_handle_order_limit_and_cycle_mode() {
    let unicode_current = window(1, r"C:\Apps\µ.exe");
    let unicode_config = [ApplicationConfiguration::new(
        0,
        "Μ.exe".into(),
        CycleMode::NextWindow,
        false,
    )];
    assert_eq!(
        get_focused_app_windows(
            Some(unicode_current.clone()),
            vec![unicode_current],
            &unicode_config
        )
        .all_windows
        .len(),
        1
    );
    let current = window(15, r"C:\Apps\App.exe");
    let mut all: Vec<_> = (1..=15)
        .rev()
        .map(|i| window(i, r"C:\Apps\app.EXE"))
        .collect();
    all.push(window(99, "other.exe"));
    let configured = [ApplicationConfiguration::new(
        0,
        "APP.exe".into(),
        CycleMode::NextWindow,
        false,
    )];
    let focused = get_focused_app_windows(Some(current.clone()), all.clone(), &configured);
    assert_eq!(
        focused
            .all_windows
            .iter()
            .map(|w| w.handle)
            .collect::<Vec<_>>(),
        (1..=10).collect::<Vec<_>>()
    );
    assert_eq!(focused.focused_window.unwrap().handle, 15);
    assert!(get_focused_app_windows(None, all.clone(), &configured)
        .focused_window
        .is_none());
    let wrong_mode = [ApplicationConfiguration::new(
        0,
        "app.exe".into(),
        CycleMode::NextApp,
        false,
    )];
    assert!(get_focused_app_windows(Some(current), all, &wrong_mode)
        .all_windows
        .is_empty());
}

#[test]
fn running_apps_are_unique_sorted_excluded_and_classified() {
    let all = [
        window(1, r"C:\Apps\Zed.exe"),
        window(2, r"C:\Apps\zed.EXE"),
        window(3, r"C:\Packages\Editor\app\Alpha.exe"),
        window(4, "OmniSpot.exe"),
        window(5, "skip.exe"),
    ];
    let result = running_applications_service::get_running_applications(
        &all,
        &[r"C:\Other\SKIP.exe".into()],
        &[r"C:\Packages\Editor".into()],
    );
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].process_name, "Alpha.exe");
    assert!(result[0].is_packaged_app);
    assert_eq!(result[1].process_name, "Zed.exe");
    assert!(!result[1].is_packaged_app);
}

#[test]
fn dynamic_keys_reject_non_letters_and_preserve_unicode_case_rules() {
    assert_eq!(app_name_resolver::get_dynamic_key("ſoftware"), Some(18));
    assert_eq!(app_name_resolver::get_dynamic_key("ı"), None);
    for (text, expected) in [
        ("spotify", Some(18)),
        ("Paint", Some(15)),
        ("", None),
        ("1password", None),
        ("_tool", None),
        ("日本", None),
    ] {
        assert_eq!(app_name_resolver::get_dynamic_key(text), expected);
    }
}

#[test]
fn process_queries_handle_current_and_missing_processes() {
    let path = process_path_extractor::get_process_image_path(std::process::id()).unwrap();
    assert!(path.ends_with(".exe"));
    assert!(process_inspector::is_windows_executable(&path));
    assert!(process_path_extractor::get_process_image_path(u32::MAX).is_none());
    assert!(!process_inspector::needs_elevation(u32::MAX));
    assert!(!process_inspector::is_windows_executable(
        r"Z:\missing-omnispot-file.exe"
    ));
    assert!(
        !process_inspector::wait_for_potential_elevation("missing-omnispot-process.exe", 0)
            .unwrap()
    );
}
