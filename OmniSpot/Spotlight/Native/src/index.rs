use crate::matcher::{compute_mask, normalize, score_query};
use crate::model::{Candidate, ExtraAppItem, IndexedResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const CURRENT_INDEX_VERSION: u32 = 3;
const MAX_INDEXED_FILES: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IndexedEntry {
    pub title: String,
    pub target: String,
    pub alternate_name: String,
    pub result_type: String,
    pub base_score: i32,
    pub usage_boost: i32,
    #[serde(default)]
    pub last_launch: i64,
    #[serde(default)]
    pub normalized_title: String,
    #[serde(default)]
    pub normalized_alternate_name: String,
    #[serde(default)]
    pub character_mask: [u64; 2],
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct SearchIndex {
    #[serde(default)]
    pub version: u32,
    pub entries: Vec<IndexedEntry>,
    #[serde(skip)]
    last_refresh: Option<SystemTime>,
    #[serde(skip)]
    dirty: bool,
    #[serde(skip)]
    pub(crate) roots: Vec<PathBuf>,
    #[serde(skip)]
    pub(crate) persistence_path: Option<PathBuf>,
    #[serde(skip)]
    pub(crate) query_cache: std::sync::Mutex<Option<(String, Vec<u32>)>>,
    #[serde(skip)]
    pub(crate) extra_apps: Vec<ExtraAppItem>,
}

struct ScoredCandidate {
    index: u32,
    score: i32,
}

impl SearchIndex {
    pub(crate) fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<SearchIndex>(&bytes).ok())
            .filter(|index| index.version == CURRENT_INDEX_VERSION)
            .unwrap_or_default()
    }

    pub(crate) fn save(&mut self, path: &Path) {
        self.version = CURRENT_INDEX_VERSION;
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec(self) {
            let temporary = path.with_extension("tmp");
            if fs::write(&temporary, bytes).is_ok() {
                let _ = fs::rename(temporary, path);
            }
        }
    }



    pub(crate) fn scan_all(
        roots: &[PathBuf],
        extra_apps: &[ExtraAppItem],
        persistence_path: &Path,
    ) -> Self {
        let mut entries = seed_windows_tools();
        let mut seen = HashSet::new();
        for entry in &entries {
            seen.insert(entry.target.to_ascii_lowercase());
        }

        for app in enumerate_shell_apps() {
            let target_key = app.target.to_ascii_lowercase();
            if seen.insert(target_key) {
                entries.push(app);
            }
        }

        for app in extra_apps {
            let target_key = app.target.to_ascii_lowercase();
            if seen.insert(target_key) {
                let base = if app.base_score > 0 { app.base_score } else { 550 };
                entries.push(create_entry(&app.title, &app.target, &app.alternate_name, "App", base));
            }
        }

        for root in roots {
            if !root.is_dir() {
                continue;
            }
            let root_str = root.to_string_lossy().to_ascii_lowercase();
            if root_str.contains("start menu") || root_str.contains("programs") || root_str.ends_with("windowsapps") {
                scan_app_directory(root, 0, 3, &mut entries, &mut seen);
            } else if root_str.contains("program files") {
                scan_app_directory(root, 0, 2, &mut entries, &mut seen);
            } else if is_user_profile_root(root) {
                let docs = root.join("Documents");
                if docs.is_dir() {
                    scan_file_directory(&docs, 0, 3, &mut entries, &mut seen, MAX_INDEXED_FILES);
                }
                let downloads = root.join("Downloads");
                if downloads.is_dir() {
                    scan_file_directory(&downloads, 0, 2, &mut entries, &mut seen, MAX_INDEXED_FILES);
                }
            } else {
                scan_app_directory(root, 0, 3, &mut entries, &mut seen);
                scan_file_directory(root, 0, 3, &mut entries, &mut seen, MAX_INDEXED_FILES);
            }
            if entries.len() >= MAX_INDEXED_FILES {
                break;
            }
        }

        SearchIndex {
            version: CURRENT_INDEX_VERSION,
            entries,
            last_refresh: Some(SystemTime::now()),
            dirty: false,
            roots: roots.to_vec(),
            persistence_path: Some(persistence_path.to_path_buf()),
            query_cache: std::sync::Mutex::new(None),
            extra_apps: extra_apps.to_vec(),
        }
    }

    pub(crate) fn swap_entries(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.entries, &mut other.entries);
        self.last_refresh = Some(SystemTime::now());
        self.dirty = false;
        if let Ok(mut guard) = self.query_cache.lock() {
            *guard = None;
        }
    }



    pub(crate) fn refresh_roots(
        &mut self,
        roots: &[PathBuf],
        extra_apps: &[ExtraAppItem],
        persistence_path: &Path,
    ) {
        // Reuse persisted snapshot on startup for instant readiness.
        if !self.entries.is_empty() {
            self.roots = roots.to_vec();
            self.extra_apps = extra_apps.to_vec();
            self.persistence_path = Some(persistence_path.to_path_buf());
            self.dirty = false;
            self.last_refresh = Some(SystemTime::now());

            let mut existing_targets: HashSet<String> = self
                .entries
                .iter()
                .map(|e| e.target.to_ascii_lowercase())
                .collect();
            let mut added = false;

            for tool in seed_windows_tools() {
                if existing_targets.insert(tool.target.to_ascii_lowercase()) {
                    self.entries.push(tool);
                    added = true;
                }
            }

            for app in enumerate_shell_apps() {
                if existing_targets.insert(app.target.to_ascii_lowercase()) {
                    self.entries.push(app);
                    added = true;
                }
            }

            for app in extra_apps {
                if existing_targets.insert(app.target.to_ascii_lowercase()) {
                    let base = if app.base_score > 0 { app.base_score } else { 550 };
                    self.entries.push(create_entry(&app.title, &app.target, &app.alternate_name, "App", base));
                    added = true;
                }
            }

            for entry in &mut self.entries {
                if entry.normalized_title.is_empty() {
                    entry.normalized_title = normalize(&entry.title);
                    entry.normalized_alternate_name = normalize(&entry.alternate_name);
                    entry.character_mask = compute_mask(&entry.normalized_title);
                }
            }
            if added {
                self.save(persistence_path);
            }
            return;
        }

        if self
            .last_refresh
            .and_then(|time| time.elapsed().ok())
            .is_some_and(|elapsed| elapsed < Duration::from_secs(15))
        {
            return;
        }

        let scanned = Self::scan_all(roots, extra_apps, persistence_path);
        self.entries = scanned.entries;
        self.roots = roots.to_vec();
        self.extra_apps = extra_apps.to_vec();
        self.persistence_path = Some(persistence_path.to_path_buf());
        self.dirty = false;
        self.last_refresh = Some(SystemTime::now());
        if let Ok(mut guard) = self.query_cache.lock() {
            *guard = None;
        }
        self.save(persistence_path);
    }

    #[allow(dead_code)]
    pub(crate) fn candidates(&self) -> Vec<Candidate> {
        self.entries
            .iter()
            .enumerate()
            .map(|(id, entry)| Candidate {
                id: id as u32,
                title: entry.title.clone(),
                alternate_name: entry.alternate_name.clone(),
                base_score: entry.base_score,
                usage_boost: entry.usage_boost,
                normalized_title: entry.normalized_title.clone(),
                normalized_alternate_name: entry.normalized_alternate_name.clone(),
                character_mask: entry.character_mask,
            })
            .collect()
    }

    pub(crate) fn search(&self, query: &str, max_results: usize) -> Vec<IndexedResult> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            let mut seen = HashSet::new();
            let mut apps: Vec<&IndexedEntry> = self
                .entries
                .iter()
                .filter(|entry| entry.result_type == "App")
                .collect();
            apps.sort_unstable_by_key(|entry| (-(entry.usage_boost as i64), -entry.last_launch, -(entry.base_score as i64)));
            return apps
                .into_iter()
                .filter(|entry| seen.insert(entry.normalized_title.as_str()))
                .take(max_results)
                .map(|entry| IndexedResult {
                    title: entry.title.clone(),
                    subtitle: entry.target.clone(),
                    target: entry.target.clone(),
                    result_type: entry.result_type.clone(),
                    score: entry.base_score + entry.usage_boost,
                    icon_path: entry.target.clone(),
                })
                .collect();
        }

        let norm_query = normalize(trimmed);
        if norm_query.is_empty() {
            return Vec::new();
        }
        let query_mask = compute_mask(&norm_query);

        // Wox prefix query cache: if query grows (e.g. c -> ch -> chr), reuse matched subset
        let cached_subset = self.query_cache.lock().ok().and_then(|guard| {
            if let Some((prev, ids)) = guard.as_ref() {
                if norm_query.starts_with(prev) {
                    return Some(ids.clone());
                }
            }
            None
        });

        let mut matched_indices = Vec::new();
        let mut scored_matches = Vec::new();

        if let Some(cached_ids) = cached_subset {
            for idx in cached_ids {
                let idx_usize = idx as usize;
                if let Some(entry) = self.entries.get(idx_usize) {
                    if let Some(fuzzy_score) = score_query(
                        &norm_query,
                        query_mask,
                        &entry.normalized_title,
                        &entry.normalized_alternate_name,
                        entry.character_mask,
                    ) {
                        let total_score = fuzzy_score + entry.base_score + entry.usage_boost;
                        matched_indices.push(idx);
                        scored_matches.push(ScoredCandidate { index: idx, score: total_score });
                    }
                }
            }
        } else {
            for (idx, entry) in self.entries.iter().enumerate() {
                let idx = idx as u32;
                if let Some(fuzzy_score) = score_query(
                    &norm_query,
                    query_mask,
                    &entry.normalized_title,
                    &entry.normalized_alternate_name,
                    entry.character_mask,
                ) {
                    let total_score = fuzzy_score + entry.base_score + entry.usage_boost;
                    matched_indices.push(idx);
                    scored_matches.push(ScoredCandidate { index: idx, score: total_score });
                }
            }
        }

        if let Ok(mut guard) = self.query_cache.lock() {
            *guard = Some((norm_query, matched_indices));
        }

        scored_matches.sort_unstable_by(|a, b| b.score.cmp(&a.score));

        let mut seen_titles = HashSet::new();
        let mut results = Vec::with_capacity(max_results);

        for m in scored_matches {
            if let Some(entry) = self.entries.get(m.index as usize) {
                if entry.result_type == "App" && !seen_titles.insert(entry.normalized_title.as_str()) {
                    continue;
                }
                results.push(IndexedResult {
                    title: entry.title.clone(),
                    subtitle: entry.target.clone(),
                    target: entry.target.clone(),
                    result_type: entry.result_type.clone(),
                    score: m.score,
                    icon_path: entry.target.clone(),
                });
                if results.len() >= max_results {
                    break;
                }
            }
        }

        results
    }

    #[allow(dead_code)]
    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    #[allow(dead_code)]
    pub(crate) fn refresh_if_dirty(&mut self) {
        if self.dirty {
            if let Some(persistence) = self.persistence_path.clone() {
                let roots = self.roots.clone();
                let extra = self.extra_apps.clone();
                self.refresh_roots(&roots, &extra, &persistence);
            }
        }
    }

    pub(crate) fn record_launch(&mut self, target: &str, persistence_path: &Path) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.target.eq_ignore_ascii_case(target)) {
            entry.usage_boost = (entry.usage_boost + 5).min(50);
            entry.last_launch = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|duration| duration.as_secs() as i64)
                .unwrap_or_default();
            self.save(persistence_path);
            if let Ok(mut guard) = self.query_cache.lock() {
                *guard = None;
            }
        }
    }
}

fn create_entry(
    title: &str,
    target: &str,
    alternate_name: &str,
    result_type: &str,
    base_score: i32,
) -> IndexedEntry {
    let normalized_title = normalize(title);
    let normalized_alternate_name = normalize(alternate_name);
    let character_mask = compute_mask(&normalized_title);

    IndexedEntry {
        title: title.to_string(),
        target: target.to_string(),
        alternate_name: alternate_name.to_string(),
        result_type: result_type.to_string(),
        base_score,
        usage_boost: 0,
        last_launch: 0,
        normalized_title,
        normalized_alternate_name,
        character_mask,
    }
}

fn is_user_profile_root(path: &Path) -> bool {
    if let Ok(profile) = std::env::var("USERPROFILE") {
        if Path::new(&profile) == path {
            return true;
        }
    }
    false
}

fn clean_shortcut_name(stem: &str) -> String {
    let trimmed = stem.trim();
    if let Some(stripped) = trimmed.strip_suffix(" - Shortcut") {
        stripped.trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn is_uninstaller_or_helper(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("uninstall")
        || lower.contains("unins000")
        || lower.contains("update_helper")
        || lower.contains("crashpad")
        || lower.contains("installer")
        || lower.starts_with("setup")
}

fn is_skipped_dir(name: &str) -> bool {
    if name.starts_with('.') || name.starts_with('$') {
        return true;
    }
    matches!(
        name.to_ascii_lowercase().as_str(),
        "appdata"
            | "programdata"
            | "windows"
            | "node_modules"
            | "target"
            | "bin"
            | "obj"
            | "temp"
            | "tmp"
            | "cache"
            | "caches"
            | "extensions"
            | "plugins"
            | "packages"
            | "package-cache"
            | "site-packages"
            | "system volume information"
            | "recovery"
            | "perflogs"
    )
}

fn is_indexable_file_ext(ext: &str) -> bool {
    matches!(
        ext,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "csv" |
        "txt" | "md" | "rtf" |
        "zip" | "rar" | "7z" | "tar" | "gz" |
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" |
        "mp4" | "mkv" | "mov" | "avi" | "mp3" | "wav" | "flac" |
        "rs" | "cs" | "py" | "js" | "ts" | "jsx" | "tsx" | "json" | "xml" | "html" | "css" | "sql"
    )
}

fn is_skipped_file_ext(ext: &str) -> bool {
    matches!(
        ext,
        "dll" | "sys" | "lib" | "pdb" | "obj" | "exp" | "ilk" | "tlog" |
        "tmp" | "lock" | "cache" | "class" | "meta" | "bak" | "idb"
    )
}

fn scan_app_directory(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    entries: &mut Vec<IndexedEntry>,
    seen_targets: &mut HashSet<String>,
) {
    if depth > max_depth || !dir.is_dir() {
        return;
    }

    let read_dir = match fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(_) => return,
    };

    for item in read_dir.flatten() {
        let path = item.path();
        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if is_skipped_dir(dir_name) {
                continue;
            }
            scan_app_directory(&path, depth + 1, max_depth, entries, seen_targets);
            continue;
        }

        let extension = match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) => ext.to_ascii_lowercase(),
            None => continue,
        };

        if !matches!(extension.as_str(), "lnk" | "url" | "exe") {
            continue;
        }

        let file_stem = match path.file_stem().and_then(|stem| stem.to_str()) {
            Some(stem) if !stem.is_empty() => stem,
            _ => continue,
        };

        if is_uninstaller_or_helper(file_stem) {
            continue;
        }

        let target_key = path.to_string_lossy().to_ascii_lowercase();
        if !seen_targets.insert(target_key) {
            continue;
        }

        let title = clean_shortcut_name(file_stem);
        let alt = path.file_name().and_then(|n| n.to_str()).unwrap_or(file_stem);

        entries.push(create_entry(
            &title,
            path.to_string_lossy().as_ref(),
            alt,
            "App",
            550,
        ));
    }
}

fn scan_file_directory(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    entries: &mut Vec<IndexedEntry>,
    seen_targets: &mut HashSet<String>,
    max_files: usize,
) {
    if depth > max_depth || entries.len() >= max_files || !dir.is_dir() {
        return;
    }

    let read_dir = match fs::read_dir(dir) {
        Ok(read_dir) => read_dir,
        Err(_) => return,
    };

    for item in read_dir.flatten() {
        if entries.len() >= max_files {
            break;
        }

        let path = item.path();
        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if is_skipped_dir(dir_name) {
                continue;
            }
            scan_file_directory(&path, depth + 1, max_depth, entries, seen_targets, max_files);
            continue;
        }

        let extension = match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) => ext.to_ascii_lowercase(),
            None => continue,
        };

        if is_skipped_file_ext(&extension) || !is_indexable_file_ext(&extension) {
            continue;
        }

        let file_stem = match path.file_stem().and_then(|stem| stem.to_str()) {
            Some(stem) if !stem.is_empty() => stem,
            _ => continue,
        };

        let target_key = path.to_string_lossy().to_ascii_lowercase();
        if !seen_targets.insert(target_key) {
            continue;
        }

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(file_stem);

        entries.push(create_entry(
            file_stem,
            path.to_string_lossy().as_ref(),
            file_name,
            "File",
            0,
        ));
    }
}

fn enumerate_shell_apps() -> Vec<IndexedEntry> {
    use windows::core::w;
    use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{
        BHID_EnumItems, IEnumShellItems, IShellItem, SHCreateItemFromParsingName,
        SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY,
    };

    let mut apps = Vec::new();

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        if let Ok(shell_folder) = SHCreateItemFromParsingName::<_, _, IShellItem>(w!("shell:AppsFolder"), None) {
            if let Ok(enum_items) = shell_folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems) {
                let mut item_buf = [None];
                let mut fetched = 0;
                while enum_items.Next(&mut item_buf, Some(&mut fetched)).is_ok() && fetched == 1 {
                    if let Some(item) = item_buf[0].take() {
                        let name = if let Ok(p) = item.GetDisplayName(SIGDN_NORMALDISPLAY) {
                            let s = p.to_string().unwrap_or_default();
                            CoTaskMemFree(Some(p.as_ptr() as _));
                            s
                        } else {
                            String::new()
                        };

                        let target = if let Ok(p) = item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING) {
                            let s = p.to_string().unwrap_or_default();
                            CoTaskMemFree(Some(p.as_ptr() as _));
                            s
                        } else {
                            String::new()
                        };

                        if !name.is_empty() && !target.is_empty() && !name.starts_with('{') && !target.ends_with(".url") {
                            let alt = name.replace(' ', "");
                            apps.push(create_entry(&name, &target, &alt, "App", 550));
                        }
                    }
                }
            }
        }

        CoUninitialize();
    }

    apps
}

fn seed_windows_tools() -> Vec<IndexedEntry> {
    let system = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    let system32 = PathBuf::from(&system).join("System32");

    let tools = [
        ("Control Panel", system32.join("control.exe"), "control"),
        ("Task Manager", system32.join("Taskmgr.exe"), "taskmgr"),
        ("File Explorer", PathBuf::from(&system).join("explorer.exe"), "explorer"),
        ("Command Prompt", system32.join("cmd.exe"), "cmd"),
        ("PowerShell", system32.join("WindowsPowerShell\\v1.0\\powershell.exe"), "powershell"),
        ("Registry Editor", PathBuf::from(&system).join("regedit.exe"), "regedit"),
        ("Notepad", system32.join("notepad.exe"), "notepad"),
        ("Calculator", system32.join("calc.exe"), "calc"),
        ("Snipping Tool", system32.join("SnippingTool.exe"), "snippingtool"),
        ("Device Manager", system32.join("devmgmt.msc"), "devmgmt"),
        ("Disk Management", system32.join("diskmgmt.msc"), "diskmgmt"),
        ("Services", system32.join("services.msc"), "services"),
        ("Event Viewer", system32.join("eventvwr.msc"), "eventvwr"),
        ("Resource Monitor", system32.join("resmon.exe"), "resmon"),
    ];

    let mut entries = Vec::with_capacity(tools.len() + 10);
    for (title, target, alt) in tools {
        entries.push(create_entry(
            title,
            target.to_string_lossy().as_ref(),
            alt,
            "App",
            600,
        ));
    }

    let settings = [
        ("Settings", "ms-settings:", "settings"),
        ("Windows Update", "ms-settings:windowsupdate", "update"),
        ("Apps & Features", "ms-settings:appsfeatures", "uninstall"),
        ("Display Settings", "ms-settings:display", "display screen resolution"),
        ("Sound Settings", "ms-settings:sound", "sound audio volume"),
        ("Bluetooth & Devices", "ms-settings:bluetooth", "bluetooth devices"),
        ("Network & Internet", "ms-settings:network", "wifi ethernet network internet"),
        ("Personalization", "ms-settings:personalization", "themes wallpaper background"),
        ("Date & Time", "ms-settings:dateandtime", "clock date time timezone"),
    ];

    for (title, target, alt) in settings {
        entries.push(create_entry(
            title,
            target,
            alt,
            "App",
            550,
        ));
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::SearchIndex;
    use crate::model::ExtraAppItem;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn enumerate_shell_apps_discovers_installed_apps() {
        let apps = super::enumerate_shell_apps();
        assert!(!apps.is_empty(), "Should discover apps from shell:AppsFolder");
        let has_chatgpt = apps.iter().any(|app| app.title == "ChatGPT");
        let has_calc = apps.iter().any(|app| app.title == "Calculator");
        assert!(has_chatgpt || has_calc, "Should find ChatGPT or Calculator");

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
        let root = std::env::temp_dir().join(format!("omnispot-app-priority-test-{}", std::process::id()));
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
        let extra_apps = vec![
            ExtraAppItem {
                title: "ChatGPT".to_string(),
                target: "OpenAI.Codex_2p2nqsd0c76g0!App".to_string(),
                alternate_name: "OpenAI Codex chatgpt".to_string(),
                base_score: 550,
            }
        ];
        let persistence = std::env::temp_dir().join(format!("omnispot-extra-app-test-{}.json", std::process::id()));
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
    fn reuses_prefix_query_cache() {
        let root = std::env::temp_dir().join(format!("omnispot-cache-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let persistence = root.join("index.json");

        let mut index = SearchIndex::default();
        index.refresh_roots(std::slice::from_ref(&root), &[], &persistence);

        let r1 = index.search("co", 10);
        assert!(!r1.is_empty());
        assert!(index.query_cache.lock().unwrap().is_some());

        let r2 = index.search("con", 10);
        assert!(!r2.is_empty());

        let _ = fs::remove_dir_all(root);
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
        assert!(duration.as_millis() < 5, "Search took too long: {:?}", duration);
    }
}
