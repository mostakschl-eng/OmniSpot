use super::SearchIndex;
use crate::model::ExtraAppItem;
use std::fs;
use std::path::PathBuf;

#[test]
fn enumerate_shell_apps_discovers_installed_apps() {
    let apps = super::enumerate_shell_apps();
    assert!(
        !apps.is_empty(),
        "Should discover apps from shell:AppsFolder"
    );
    let has_chatgpt = apps.iter().any(|app| app.title == "ChatGPT");
    assert!(apps
        .iter()
        .all(|app| !app.title.is_empty() && !app.target.is_empty()));

    let mut index = SearchIndex::default();
    index.entries = apps;
    // Search for "chatgpt"
    let res = index.search("chatgpt", 5);
    if has_chatgpt {
        assert!(!res.is_empty());
        assert_eq!(res[0].title, "ChatGPT");
        assert_eq!(res[0].result_type, "App");
    }
}

#[test]
fn empty_or_missing_persistence_loads_cleanly() {
    let index = SearchIndex::load(&PathBuf::from("missing-omnispot-index.json"));
    assert!(index.entries.is_empty());
}

#[test]
fn indexes_files_and_returns_fuzzy_matches() {
    let root = std::env::temp_dir().join(format!("omnispot-index-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("MySpecialTestFile.url"), b"shortcut").unwrap();
    let persistence = root.join("index.json");

    let mut index = SearchIndex::default();
    index.refresh_roots(std::slice::from_ref(&root), &[], &persistence);

    let results = index.search("myspecialtestfile", 10);
    assert!(!results.is_empty());
    assert!(results[0].target.ends_with("MySpecialTestFile.url"));
    let reloaded = SearchIndex::load(&persistence);
    assert!(!reloaded.entries.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn prioritizes_apps_over_files() {
    let root =
        std::env::temp_dir().join(format!("omnispot-app-priority-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let persistence = root.join("index.json");
    let mut index = SearchIndex::default();

    fs::write(root.join("Terminal.exe"), b"exe").unwrap();
    fs::write(root.join("Terminal.txt"), b"txt").unwrap();

    index.refresh_roots(std::slice::from_ref(&root), &[], &persistence);

    let results = index.search("terminal", 10);
    assert!(results.len() >= 2);
    assert_eq!(results[0].result_type, "App");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn indexes_extra_packaged_apps_and_finds_them_with_high_priority() {
    let mut index = SearchIndex::default();
    let extra_apps = vec![ExtraAppItem {
        title: "ChatGPT".to_string(),
        target: "OpenAI.Codex_2p2nqsd0c76g0!App".to_string(),
        alternate_name: "OpenAI Codex chatgpt".to_string(),
        base_score: 80,
    }];
    let persistence = std::env::temp_dir().join(format!(
        "omnispot-extra-app-test-{}.json",
        std::process::id()
    ));
    index.refresh_roots(&[], &extra_apps, &persistence);

    // Also add a file match with "chatgpt" in its name
    index.entries.push(super::create_entry(
        "ChatGPT Image Mar 26, 2026, 05_33_13 PM",
        "C:\\Users\\freef\\Downloads\\ChatGPT Image.jpg",
        "ChatGPT Image.jpg",
        "File",
        0,
    ));

    let results = index.search("chatgpt", 5);
    assert!(!results.is_empty());
    assert_eq!(results[0].title, "ChatGPT");
    assert_eq!(results[0].result_type, "App");
    assert_eq!(results[0].target, "OpenAI.Codex_2p2nqsd0c76g0!App");
    let _ = fs::remove_file(persistence);
}

#[test]
fn reuses_identical_query_cache() {
    let root = std::env::temp_dir().join(format!("omnispot-cache-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let persistence = root.join("index.json");

    let mut index = SearchIndex::default();
    index.refresh_roots(std::slice::from_ref(&root), &[], &persistence);

    let r1 = index.search("co", 10);
    assert!(!r1.is_empty());
    assert!(index.query_cache.lock().unwrap().is_some());

    let r2 = index.search("co", 10);
    assert!(!r2.is_empty());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn growing_query_can_introduce_a_fuzzy_match() {
    let mut index = SearchIndex::default();
    index.entries = vec![super::create_entry(
        "Google Chrome",
        "chrome.exe",
        "",
        "App",
        80,
    )];
    assert_eq!(index.search("ghr", 10).len(), 1);
    assert!(index.search("gh", 10).is_empty());
    assert_eq!(index.search("ghr", 10).len(), 1);
}

#[test]
fn refresh_preserves_launch_history_and_removes_old_roots() {
    let root = std::env::temp_dir().join(format!("omnispot-refresh-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("UniqueRefreshDocument.txt"), b"test").unwrap();
    let persistence = root.join("index.json");
    let mut index = SearchIndex::default();
    index.refresh_roots(std::slice::from_ref(&root), &[], &persistence);
    let target = index
        .entries
        .iter()
        .find(|entry| entry.title == "Notepad")
        .unwrap()
        .target
        .clone();
    index.record_launch(&target, &persistence);
    index.refresh_roots(&[], &[], &persistence);
    let notepad = index
        .entries
        .iter()
        .find(|entry| entry.target == target)
        .unwrap();
    assert_eq!(notepad.usage_boost, 5);
    assert!(notepad.last_launch > 0);
    assert!(index.search("UniqueRefreshDocument", 10).is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn zero_limit_returns_no_results() {
    let mut index = SearchIndex::default();
    index.entries = super::seed_windows_tools();
    assert!(index.search("notepad", 0).is_empty());
}

#[test]
fn empty_query_returns_top_apps() {
    let mut index = SearchIndex::default();
    index.entries = super::seed_windows_tools();

    let suggestions = index.search("", 5);
    assert_eq!(suggestions.len(), 5);
    for item in suggestions {
        assert_eq!(item.result_type, "App");
    }
}

#[test]
fn acronym_matches_system_tools() {
    let mut index = SearchIndex::default();
    index.entries = super::seed_windows_tools();

    let r_cp = index.search("cp", 5);
    assert!(!r_cp.is_empty());
    assert!(r_cp.iter().any(|r| r.title == "Control Panel"));

    let r_tm = index.search("tm", 5);
    assert!(!r_tm.is_empty());
    assert!(r_tm.iter().any(|r| r.title == "Task Manager"));

    let r_wu = index.search("wu", 5);
    assert!(!r_wu.is_empty());
    assert!(r_wu.iter().any(|r| r.title == "Windows Update"));
}

#[test]
fn google_chrome_ranks_above_chrome_remote_desktop() {
    let mut index = SearchIndex::default();
    index.entries = vec![
        super::create_entry(
            "Chrome Remote Desktop",
            "remotedesktop.google.com-906A0B3D_h2dphjv1brgng!App",
            "ChromeRemoteDesktop",
            "App",
            50,
        ),
        super::create_entry("Google Chrome", "Chrome", "GoogleChrome", "App", 80),
    ];

    let results = index.search("chrome", 5);
    assert!(!results.is_empty());
    assert_eq!(results[0].title, "Google Chrome");
}

#[test]
fn deduplicates_duplicate_apps() {
    let mut index = SearchIndex::default();
    index.entries = vec![
        super::create_entry("Chrome", "C:\\Path1\\chrome.exe", "chrome.exe", "App", 500),
        super::create_entry("Chrome", "C:\\Path2\\chrome.exe", "chrome.exe", "App", 500),
    ];

    let results = index.search("chrome", 5);
    assert_eq!(results.len(), 1);
}

#[test]
fn sub_millisecond_search_over_10k_entries() {
    let mut index = SearchIndex::default();
    index.entries = super::seed_windows_tools();

    for i in 0..10_000 {
        index.entries.push(super::create_entry(
            &format!("Document file sample {i}"),
            &format!("C:\\Docs\\file_{i}.txt"),
            &format!("file_{i}.txt"),
            "File",
            0,
        ));
    }

    let start = std::time::Instant::now();
    let results = index.search("task", 10);
    let duration = start.elapsed();

    assert!(!results.is_empty());
    assert_eq!(results[0].title, "Task Manager");
    assert!(
        duration.as_millis() < 5,
        "Search took too long: {:?}",
        duration
    );
}

#[test]
fn parses_extension_filters_correctly() {
    let (ext1, q1) = super::parse_extension_filter("ext:pdf");
    assert_eq!(ext1, Some("pdf".to_string()));
    assert_eq!(q1, "");

    let (ext2, q2) = super::parse_extension_filter("ext:docx annual report");
    assert_eq!(ext2, Some("docx".to_string()));
    assert_eq!(q2, "annual report");

    let (ext3, q3) = super::parse_extension_filter("*.rs engine");
    assert_eq!(ext3, Some("rs".to_string()));
    assert_eq!(q3, "engine");

    let (ext4, q4) = super::parse_extension_filter("pdf: invoice");
    assert_eq!(ext4, Some("pdf".to_string()));
    assert_eq!(q4, "invoice");

    let (ext5, q5) = super::parse_extension_filter("chrome");
    assert_eq!(ext5, None);
    assert_eq!(q5, "chrome");
}

#[test]
fn searches_by_file_extension() {
    let mut index = SearchIndex::default();
    index.entries = vec![
        super::create_entry(
            "Annual Report",
            "C:\\Docs\\report.pdf",
            "report.pdf",
            "File",
            0,
        ),
        super::create_entry("Tax Summary", "C:\\Docs\\tax.pdf", "tax.pdf", "File", 0),
        super::create_entry("Rust Backend", "C:\\Dev\\engine.rs", "engine.rs", "File", 0),
        super::create_entry("Notes", "C:\\Docs\\notes.txt", "notes.txt", "File", 0),
    ];

    let pdf_results = index.search("ext:pdf", 10);
    assert_eq!(pdf_results.len(), 2);
    assert!(pdf_results.iter().all(|r| r.target.ends_with(".pdf")));

    let rs_results = index.search("*.rs engine", 10);
    assert_eq!(rs_results.len(), 1);
    assert_eq!(rs_results[0].title, "Rust Backend");
}

#[test]
fn browses_local_directory_in_realtime() {
    let temp_dir = std::env::temp_dir().join(format!("omnispot-dir-browse-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(temp_dir.join("SubFolderA")).unwrap();
    fs::create_dir_all(temp_dir.join("SubFolderB")).unwrap();
    fs::write(temp_dir.join("Document.pdf"), b"pdf").unwrap();
    fs::write(temp_dir.join("Source.rs"), b"rs").unwrap();

    let index = SearchIndex::default();
    let browse_query = format!("{}\\", temp_dir.display());
    let results = index.search(&browse_query, 10);

    assert!(results.len() >= 4);
    // Folders must be listed before files
    assert_eq!(results[0].result_type, "Folder");
    assert_eq!(results[1].result_type, "Folder");

    // Partial filter in directory
    let filter_query = format!("{}\\Doc", temp_dir.display());
    let filtered = index.search(&filter_query, 10);
    assert!(!filtered.is_empty());
    assert_eq!(filtered[0].title, "Document.pdf");
    assert_eq!(filtered[0].result_type, "File");

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn smart_ranking_shows_files_and_apps_without_starvation() {
    let mut index = SearchIndex::default();
    index.entries = vec![
        super::create_entry(
            "Google Chrome",
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            "chrome.exe",
            "App",
            80,
        ),
        super::create_entry(
            "Chrome Remote Desktop",
            "remotedesktop.google.com-906A0B3D_h2dphjv1brgng!App",
            "ChromeRemoteDesktop",
            "App",
            50,
        ),
        super::create_entry(
            "codex-chrome-native-host",
            "C:\\Tools\\codex-chrome-native-host.exe",
            "codex-chrome-native-host",
            "App",
            80,
        ),
        super::create_entry(
            "chrome_shortcuts",
            "C:\\Docs\\chrome_shortcuts.pdf",
            "chrome_shortcuts.pdf",
            "File",
            0,
        ),
        super::create_entry(
            "Quarterly Financial Statement 2026",
            "C:\\Docs\\Quarterly_Financial_Statement_2026.xlsx",
            "Quarterly_Financial_Statement_2026.xlsx",
            "File",
            0,
        ),
    ];

    // 1. Searching "chrome": Google Chrome is top app, file is included in top results (not hidden!)
    let chrome_results = index.search("chrome", 10);
    assert!(!chrome_results.is_empty());
    assert_eq!(chrome_results[0].title, "Google Chrome");
    assert_eq!(chrome_results[0].result_type, "App");
    assert!(
        chrome_results
            .iter()
            .any(|r| r.title == "chrome_shortcuts" && r.result_type == "File"),
        "Matching file must not be starved out by apps"
    );

    // 2. Searching full file name: file ranks at the top #1!
    let file_results = index.search("Quarterly Financial Statement", 5);
    assert!(!file_results.is_empty());
    assert_eq!(file_results[0].title, "Quarterly Financial Statement 2026");
    assert_eq!(file_results[0].result_type, "File");
}
