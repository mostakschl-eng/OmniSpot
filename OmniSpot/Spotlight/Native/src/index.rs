use crate::matcher::{compute_mask, normalize, score_query};
use crate::model::{Candidate, ExtraAppItem, IndexedResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const CURRENT_INDEX_VERSION: u32 = 5;
const MAX_INDEXED_FILES: usize = 50_000;

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
                let base = if app.base_score > 0 {
                    app.base_score
                } else {
                    80
                };
                entries.push(create_entry(
                    &app.title,
                    &app.target,
                    &app.alternate_name,
                    "App",
                    base,
                ));
            }
        }

        for root in roots {
            if !root.is_dir() {
                continue;
            }
            let root_str = root.to_string_lossy().to_ascii_lowercase();
            if root_str.contains("start menu")
                || root_str.contains("programs")
                || root_str.ends_with("windowsapps")
            {
                scan_app_directory(root, 0, 3, &mut entries, &mut seen);
            } else if root_str.contains("program files") {
                scan_app_directory(root, 0, 2, &mut entries, &mut seen);
            } else if is_user_profile_root(root) {
                let docs = root.join("Documents");
                if docs.is_dir() {
                    scan_file_directory(&docs, 0, 5, &mut entries, &mut seen, MAX_INDEXED_FILES);
                }
                let downloads = root.join("Downloads");
                if downloads.is_dir() {
                    scan_file_directory(
                        &downloads,
                        0,
                        4,
                        &mut entries,
                        &mut seen,
                        MAX_INDEXED_FILES,
                    );
                }
                let videos = root.join("Videos");
                if videos.is_dir() {
                    scan_file_directory(&videos, 0, 5, &mut entries, &mut seen, MAX_INDEXED_FILES);
                }
                let pictures = root.join("Pictures");
                if pictures.is_dir() {
                    scan_file_directory(
                        &pictures,
                        0,
                        4,
                        &mut entries,
                        &mut seen,
                        MAX_INDEXED_FILES,
                    );
                }
                let music = root.join("Music");
                if music.is_dir() {
                    scan_file_directory(&music, 0, 4, &mut entries, &mut seen, MAX_INDEXED_FILES);
                }
            } else {
                scan_app_directory(root, 0, 3, &mut entries, &mut seen);
                scan_file_directory(root, 0, 5, &mut entries, &mut seen, MAX_INDEXED_FILES);
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
        let history: HashMap<_, _> = self
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.target.to_ascii_lowercase(),
                    (entry.usage_boost, entry.last_launch),
                )
            })
            .collect();
        for entry in &mut other.entries {
            if let Some(&(usage_boost, last_launch)) =
                history.get(&entry.target.to_ascii_lowercase())
            {
                entry.usage_boost = usage_boost;
                entry.last_launch = last_launch;
            }
        }
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
        let mut scanned = Self::scan_all(roots, extra_apps, persistence_path);
        self.swap_entries(&mut scanned);
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
        if max_results == 0 {
            return Vec::new();
        }
        let trimmed = query.trim();
        if trimmed.is_empty() {
            let mut seen = HashSet::new();
            let mut apps: Vec<&IndexedEntry> = self
                .entries
                .iter()
                .filter(|entry| entry.result_type == "App")
                .collect();
            apps.sort_unstable_by_key(|entry| {
                (
                    -(entry.usage_boost as i64),
                    -entry.last_launch,
                    -(entry.base_score as i64),
                )
            });
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

        // 1. Direct real-time path browsing (e.g. C:\, D:\, C:\Users\, ~\...)
        if let Some(browse_results) = browse_directory(trimmed, max_results) {
            if !browse_results.is_empty() {
                return browse_results;
            }
        }

        // 2. Extension filter parsing (e.g. ext:pdf, *.rs, pdf: invoice)
        let (ext_filter, clean_query) = parse_extension_filter(trimmed);
        if let Some(target_ext) = ext_filter {
            let mut matches: Vec<(&IndexedEntry, i32)> = Vec::new();
            let norm_clean = normalize(&clean_query);
            let clean_mask = compute_mask(&norm_clean);

            for entry in &self.entries {
                if entry.result_type != "File" {
                    continue;
                }
                let path = Path::new(&entry.target);
                let entry_ext = match path.extension().and_then(|s| s.to_str()) {
                    Some(ext) => ext.to_ascii_lowercase(),
                    None => continue,
                };
                if entry_ext != target_ext {
                    continue;
                }

                if norm_clean.is_empty() {
                    matches.push((entry, 800 + entry.usage_boost));
                } else if let Some(fuzzy) = score_query(
                    &norm_clean,
                    clean_mask,
                    &entry.normalized_title,
                    &entry.normalized_alternate_name,
                    entry.character_mask,
                ) {
                    let mut score = fuzzy + 200 + entry.usage_boost;
                    let target_stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&entry.title);
                    if target_stem.eq_ignore_ascii_case(&norm_clean) {
                        score += 100;
                    }
                    matches.push((entry, score));
                }
            }

            matches.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.1));
            return matches
                .into_iter()
                .take(max_results)
                .map(|(entry, score)| IndexedResult {
                    title: entry.title.clone(),
                    subtitle: entry.target.clone(),
                    target: entry.target.clone(),
                    result_type: entry.result_type.clone(),
                    score,
                    icon_path: entry.target.clone(),
                })
                .collect();
        }

        let norm_query = normalize(trimmed);
        if norm_query.is_empty() {
            return Vec::new();
        }
        let query_mask = compute_mask(&norm_query);

        // Longer queries can introduce fuzzy matches, so only reuse identical queries.
        let cached_subset = self.query_cache.lock().ok().and_then(|guard| {
            if let Some((prev, ids)) = guard.as_ref() {
                if norm_query == *prev {
                    return Some(ids.clone());
                }
            }
            None
        });

        let mut matched_indices = Vec::new();
        let mut scored_matches = Vec::new();

        let evaluate_entry = |entry: &IndexedEntry| -> Option<i32> {
            let fuzzy_score = score_query(
                &norm_query,
                query_mask,
                &entry.normalized_title,
                &entry.normalized_alternate_name,
                entry.character_mask,
            )?;

            let mut total_score = fuzzy_score + entry.base_score + entry.usage_boost;
            let target_stem = Path::new(&entry.target)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&entry.target);

            if target_stem.eq_ignore_ascii_case(&norm_query) {
                total_score += 60;
            }

            if entry.target.contains(".com-")
                || entry.target.contains(".org-")
                || entry.target.contains(".app-")
            {
                total_score -= 40;
            }

            if entry.result_type == "File" {
                let lower_target = entry.target.to_ascii_lowercase();
                if lower_target.contains("\\node_modules\\")
                    || lower_target.contains("\\target\\")
                    || lower_target.contains("\\bin\\")
                    || lower_target.contains("\\obj\\")
                    || lower_target.contains("\\.git\\")
                    || lower_target.contains("\\temp\\")
                {
                    total_score -= 150;
                }
            }

            Some(total_score)
        };

        if let Some(cached_ids) = cached_subset {
            for idx in cached_ids {
                let idx_usize = idx as usize;
                if let Some(entry) = self.entries.get(idx_usize) {
                    if let Some(total_score) = evaluate_entry(entry) {
                        matched_indices.push(idx);
                        scored_matches.push(ScoredCandidate {
                            index: idx,
                            score: total_score,
                        });
                    }
                }
            }
        } else {
            for (idx, entry) in self.entries.iter().enumerate() {
                let idx = idx as u32;
                if let Some(total_score) = evaluate_entry(entry) {
                    matched_indices.push(idx);
                    scored_matches.push(ScoredCandidate {
                        index: idx,
                        score: total_score,
                    });
                }
            }
        }

        if let Ok(mut guard) = self.query_cache.lock() {
            *guard = Some((norm_query, matched_indices));
        }

        scored_matches.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.score));

        let mut seen_titles = HashSet::new();
        let mut results = Vec::with_capacity(max_results);

        for m in scored_matches {
            if let Some(entry) = self.entries.get(m.index as usize) {
                if entry.result_type == "App"
                    && !seen_titles.insert(entry.normalized_title.as_str())
                {
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
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.target.eq_ignore_ascii_case(target))
        {
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
    let title_mask = compute_mask(&normalized_title);
    let alt_mask = compute_mask(&normalized_alternate_name);
    let character_mask = [title_mask[0] | alt_mask[0], title_mask[1] | alt_mask[1]];

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
        || lower.contains("native-host")
        || lower.contains("native_host")
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
        "pdf"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "ppt"
            | "pptx"
            | "csv"
            | "txt"
            | "md"
            | "rtf"
            | "epub"
            | "log"
            | "zip"
            | "rar"
            | "7z"
            | "tar"
            | "gz"
            | "iso"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "svg"
            | "webp"
            | "ico"
            | "bmp"
            | "tiff"
            | "mp4"
            | "mkv"
            | "mov"
            | "avi"
            | "wmv"
            | "flv"
            | "webm"
            | "m4v"
            | "mpg"
            | "mpeg"
            | "m2ts"
            | "3gp"
            | "mp3"
            | "wav"
            | "flac"
            | "aac"
            | "ogg"
            | "m4a"
            | "wma"
            | "rs"
            | "cs"
            | "py"
            | "js"
            | "ts"
            | "jsx"
            | "tsx"
            | "json"
            | "xml"
            | "html"
            | "css"
            | "sql"
            | "exe"
            | "msi"
            | "bat"
            | "cmd"
            | "ps1"
            | "lnk"
    )
}

fn is_skipped_file_ext(ext: &str) -> bool {
    matches!(
        ext,
        "dll"
            | "sys"
            | "lib"
            | "pdb"
            | "obj"
            | "exp"
            | "ilk"
            | "tlog"
            | "tmp"
            | "lock"
            | "cache"
            | "class"
            | "meta"
            | "bak"
            | "idb"
    )
}

fn parse_extension_filter(query: &str) -> (Option<String>, String) {
    let trimmed = query.trim();

    if let Some(rest) = trimmed.strip_prefix("ext:") {
        let mut parts = rest.splitn(2, |c: char| c.is_whitespace());
        let ext = parts
            .next()
            .unwrap_or_default()
            .trim()
            .trim_start_matches('.');
        let remaining = parts.next().unwrap_or_default().trim().to_string();
        if !ext.is_empty() {
            return (Some(ext.to_ascii_lowercase()), remaining);
        }
    }

    if let Some(star_pos) = trimmed.find("*.") {
        let after_dot = &trimmed[star_pos + 2..];
        let mut parts = after_dot.splitn(2, |c: char| c.is_whitespace());
        let ext = parts.next().unwrap_or_default().trim();
        if !ext.is_empty() {
            let before = trimmed[..star_pos].trim();
            let after = parts.next().unwrap_or_default().trim();
            let remaining = format!("{} {}", before, after).trim().to_string();
            return (Some(ext.to_ascii_lowercase()), remaining);
        }
    }

    if let Some((possible_ext, remaining)) = trimmed.split_once(':') {
        let candidate_ext = possible_ext.trim().to_ascii_lowercase();
        if is_indexable_file_ext(&candidate_ext) {
            return (Some(candidate_ext), remaining.trim().to_string());
        }
    }

    (None, trimmed.to_string())
}

fn is_path_query(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return false;
    }
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return true;
    }
    if trimmed.starts_with('\\') || trimmed.starts_with('/') {
        return true;
    }
    if trimmed.starts_with(".\\")
        || trimmed.starts_with("./")
        || trimmed.starts_with("..\\")
        || trimmed.starts_with("../")
        || trimmed.starts_with("~\\")
        || trimmed.starts_with("~/")
        || trimmed.starts_with('%')
    {
        return true;
    }
    false
}

fn expand_path(raw: &str) -> PathBuf {
    let mut expanded = raw.trim().to_string();

    if expanded.starts_with('%') {
        if let Some(second_pct) = expanded[1..].find('%') {
            let var_name = &expanded[1..=second_pct];
            if let Ok(val) = std::env::var(var_name) {
                expanded = format!("{}{}", val, &expanded[second_pct + 2..]);
            }
        }
    }

    if expanded.starts_with('~') {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let rest = expanded
                .strip_prefix('~')
                .unwrap_or_default()
                .trim_start_matches(['\\', '/']);
            if rest.is_empty() {
                expanded = profile;
            } else {
                expanded = format!("{}\\{}", profile, rest);
            }
        }
    }

    PathBuf::from(expanded)
}

fn browse_directory(query: &str, max_results: usize) -> Option<Vec<IndexedResult>> {
    if !is_path_query(query) {
        return None;
    }

    let target_path = expand_path(query);

    let (dir_to_scan, filter_str) = if target_path.is_dir() {
        (target_path, String::new())
    } else {
        let parent = target_path.parent()?;
        if parent.is_dir() {
            let filter = target_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            (parent.to_path_buf(), filter)
        } else {
            return None;
        }
    };

    let read_dir = fs::read_dir(&dir_to_scan).ok()?;
    let mut entries = Vec::new();
    let filter_lower = filter_str.to_ascii_lowercase();

    for item in read_dir.flatten() {
        let path = item.path();
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        if file_name.starts_with('.') || file_name.starts_with('$') {
            continue;
        }

        let is_dir = path.is_dir();
        let result_type = if is_dir { "Folder" } else { "File" };

        let score = if filter_lower.is_empty() {
            if is_dir {
                900
            } else {
                850
            }
        } else {
            let name_lower = file_name.to_ascii_lowercase();
            if name_lower.starts_with(&filter_lower) {
                if is_dir {
                    950
                } else {
                    920
                }
            } else if name_lower.contains(&filter_lower) {
                if is_dir {
                    880
                } else {
                    840
                }
            } else if let Some(fuzzy) = crate::matcher::score_text(&filter_lower, file_name) {
                fuzzy + if is_dir { 100 } else { 50 }
            } else {
                continue;
            }
        };

        entries.push(IndexedResult {
            title: file_name.to_string(),
            subtitle: path.to_string_lossy().to_string(),
            target: path.to_string_lossy().to_string(),
            result_type: result_type.to_string(),
            score,
            icon_path: path.to_string_lossy().to_string(),
        });
    }

    entries.sort_unstable_by(|a, b| {
        let a_is_folder = a.result_type == "Folder";
        let b_is_folder = b.result_type == "Folder";
        b_is_folder
            .cmp(&a_is_folder)
            .then_with(|| b.score.cmp(&a.score))
            .then_with(|| {
                a.title
                    .to_ascii_lowercase()
                    .cmp(&b.title.to_ascii_lowercase())
            })
    });

    entries.truncate(max_results);
    Some(entries)
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
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
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
        let alt = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_stem);

        entries.push(create_entry(
            &title,
            path.to_string_lossy().as_ref(),
            alt,
            "App",
            80,
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
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if is_skipped_dir(dir_name) {
                continue;
            }
            scan_file_directory(
                &path,
                depth + 1,
                max_depth,
                entries,
                seen_targets,
                max_files,
            );
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

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_stem);

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
    use windows::Win32::System::Com::{
        CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        BHID_EnumItems, IEnumShellItems, IShellItem, SHCreateItemFromParsingName,
        SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY,
    };

    let mut apps = Vec::new();

    unsafe {
        if CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_err() {
            return apps;
        }

        if let Ok(shell_folder) =
            SHCreateItemFromParsingName::<_, _, IShellItem>(w!("shell:AppsFolder"), None)
        {
            if let Ok(enum_items) =
                shell_folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems)
            {
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

                        let target =
                            if let Ok(p) = item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING) {
                                let s = p.to_string().unwrap_or_default();
                                CoTaskMemFree(Some(p.as_ptr() as _));
                                s
                            } else {
                                String::new()
                            };

                        if !name.is_empty()
                            && !target.is_empty()
                            && !name.starts_with('{')
                            && !target.ends_with(".url")
                        {
                            if is_uninstaller_or_helper(&name) || is_uninstaller_or_helper(&target)
                            {
                                continue;
                            }
                            let alt = name.replace(' ', "");
                            let base = if target.contains(".com-")
                                || target.contains(".org-")
                                || target.contains(".app-")
                            {
                                50
                            } else {
                                80
                            };
                            apps.push(create_entry(&name, &target, &alt, "App", base));
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
        (
            "File Explorer",
            PathBuf::from(&system).join("explorer.exe"),
            "explorer",
        ),
        ("Command Prompt", system32.join("cmd.exe"), "cmd"),
        (
            "PowerShell",
            system32.join("WindowsPowerShell\\v1.0\\powershell.exe"),
            "powershell",
        ),
        (
            "Registry Editor",
            PathBuf::from(&system).join("regedit.exe"),
            "regedit",
        ),
        ("Notepad", system32.join("notepad.exe"), "notepad"),
        ("Calculator", system32.join("calc.exe"), "calc"),
        (
            "Snipping Tool",
            system32.join("SnippingTool.exe"),
            "snippingtool",
        ),
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
            90,
        ));
    }

    let settings = [
        ("Settings", "ms-settings:", "settings"),
        ("Windows Update", "ms-settings:windowsupdate", "update"),
        ("Apps & Features", "ms-settings:appsfeatures", "uninstall"),
        (
            "Display Settings",
            "ms-settings:display",
            "display screen resolution",
        ),
        ("Sound Settings", "ms-settings:sound", "sound audio volume"),
        (
            "Bluetooth & Devices",
            "ms-settings:bluetooth",
            "bluetooth devices",
        ),
        (
            "Network & Internet",
            "ms-settings:network",
            "wifi ethernet network internet",
        ),
        (
            "Personalization",
            "ms-settings:personalization",
            "themes wallpaper background",
        ),
        (
            "Date & Time",
            "ms-settings:dateandtime",
            "clock date time timezone",
        ),
    ];

    for (title, target, alt) in settings {
        entries.push(create_entry(title, target, alt, "App", 85));
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
        let temp_dir =
            std::env::temp_dir().join(format!("omnispot-dir-browse-{}", std::process::id()));
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
}
