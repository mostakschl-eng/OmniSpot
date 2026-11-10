mod paths;
mod scan;
mod windows_apps;

use paths::{browse_directory, parse_extension_filter};
use scan::{is_user_profile_root, scan_app_directory, scan_file_directory};
use windows_apps::{enumerate_shell_apps, seed_windows_tools};

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

#[cfg(test)]
#[path = "../tests/index_tests.rs"]
mod tests;
