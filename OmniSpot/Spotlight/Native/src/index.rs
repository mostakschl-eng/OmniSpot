use crate::model::{Candidate, IndexedResult};
use crate::rank_candidates;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const MAX_INDEXED_FILES: usize = 250_000;

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
    pub entries: Vec<IndexedEntry>,
    #[serde(skip)]
    last_refresh: Option<SystemTime>,
    #[serde(skip)]
    dirty: bool,
    #[serde(skip)]
    roots: Vec<PathBuf>,
    #[serde(skip)]
    persistence_path: Option<PathBuf>,
    #[serde(skip)]
    query_cache: Option<(String, Vec<u32>)>,
}

impl SearchIndex {
    pub(crate) fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub(crate) fn save(&self, path: &Path) {
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

    pub(crate) fn refresh_roots(&mut self, roots: &[PathBuf], persistence_path: &Path) {
        // Reuse the persisted snapshot on startup. Full recursive scans must
        // never run on the first Spotlight query.
        if !self.entries.is_empty() {
            self.roots = roots.to_vec();
            self.persistence_path = Some(persistence_path.to_path_buf());
            self.dirty = false;
            self.last_refresh = Some(SystemTime::now());
            for entry in &mut self.entries {
                if entry.normalized_title.is_empty() {
                    prepare_entry(entry);
                }
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

        let mut entries = seed_windows_tools();
        let mut seen = HashSet::new();
        for entry in &entries {
            seen.insert(entry.target.to_lowercase());
        }
        for root in roots {
            scan_directory(root, &mut entries, &mut seen);
            if entries.len() >= MAX_INDEXED_FILES {
                break;
            }
        }
        self.entries = entries;
        for entry in &mut self.entries {
            prepare_entry(entry);
        }
        self.roots = roots.to_vec();
        self.persistence_path = Some(persistence_path.to_path_buf());
        self.dirty = false;
        self.last_refresh = Some(SystemTime::now());
        self.save(persistence_path);
    }

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
        if query.trim().is_empty() {
            let mut seen = HashSet::new();
            let mut apps = self
                .entries
                .iter()
                .filter(|entry| entry.result_type == "App")
                .collect::<Vec<_>>();
            apps.sort_by_key(|entry| (-(entry.usage_boost as i64), -entry.last_launch, -(entry.base_score as i64)));
            return apps.into_iter().filter_map(|entry| {
                if !seen.insert(entry.normalized_title.clone()) { return None; }
                Some(IndexedResult {
                    title: entry.title.clone(),
                    subtitle: entry.target.clone(),
                    target: entry.target.clone(),
                    result_type: entry.result_type.clone(),
                    score: entry.base_score + entry.usage_boost,
                    icon_path: entry.target.clone(),
                })
            }).take(max_results).collect();
        }

        let normalized_query = normalize_key(query);
        let reusable_ids = self.query_cache.as_ref().and_then(|(previous, ids)| {
            normalized_query.starts_with(previous).then_some(ids)
        });
        let candidates = self.candidates();
        let candidates = reusable_ids.map_or_else(
            || candidates,
            |ids| candidates.into_iter().filter(|candidate| ids.contains(&candidate.id)).collect(),
        );
        let mut seen_titles = HashSet::new();
        rank_candidates(query, &candidates, max_results.saturating_mul(3))
            .into_iter()
            .filter_map(|matched| {
                let entry = self.entries.get(matched.id as usize)?;
                if entry.result_type == "App" && !seen_titles.insert(entry.normalized_title.clone()) { return None; }
                Some(IndexedResult {
                    title: entry.title.clone(),
                    subtitle: entry.target.clone(),
                    target: entry.target.clone(),
                    result_type: entry.result_type.clone(),
                    score: matched.score,
                    icon_path: entry.target.clone(),
                })
            })
            .take(max_results)
            .collect()
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn refresh_if_dirty(&mut self) {
        // Keep search latency independent from filesystem change bursts. The
        // current persisted snapshot remains valid while a refresh is pending.
    }

    pub(crate) fn record_launch(&mut self, target: &str, persistence_path: &Path) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.target.eq_ignore_ascii_case(target)) {
            entry.usage_boost = (entry.usage_boost + 2).min(12);
            entry.last_launch = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                .map(|duration| duration.as_secs() as i64).unwrap_or_default();
            self.save(persistence_path);
            self.query_cache = None;
        }
    }
}

fn scan_directory(root: &Path, entries: &mut Vec<IndexedEntry>, seen: &mut HashSet<String>) {
    if entries.len() >= MAX_INDEXED_FILES || !root.is_dir() {
        return;
    }

    let read_dir = match fs::read_dir(root) {
        Ok(read_dir) => read_dir,
        Err(_) => return,
    };

    for item in read_dir.flatten() {
        if entries.len() >= MAX_INDEXED_FILES {
            break;
        }

        let path = item.path();
        let key = path.to_string_lossy().to_lowercase();
        if !seen.insert(key) {
            continue;
        }

        if path.is_dir() {
            scan_directory(&path, entries, seen);
            continue;
        }

        let title = path
            .file_stem()
            .or_else(|| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        if title.is_empty() {
            continue;
        }

        entries.push(IndexedEntry {
            title,
            target: path.to_string_lossy().into_owned(),
            alternate_name: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string(),
            result_type: if is_app_path(&path) { "App" } else { "File" }.to_string(),
            base_score: 0,
            usage_boost: 0,
            last_launch: 0,
            normalized_title: String::new(),
            normalized_alternate_name: String::new(),
            character_mask: [0, 0],
        });
    }
}

fn prepare_entry(entry: &mut IndexedEntry) {
    entry.normalized_title = normalize_key(&entry.title);
    entry.normalized_alternate_name = normalize_key(&entry.alternate_name);
    entry.character_mask = character_mask(&entry.normalized_title);
}

fn normalize_key(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.trim().chars() {
        if matches!(character, ' ' | '\t' | '_' | '-' | '.') {
            pending_space = true;
            continue;
        }
        if pending_space && !output.is_empty() {
            output.push(' ');
        }
        pending_space = false;
        output.extend(character.to_lowercase());
    }
    output
}

fn character_mask(value: &str) -> [u64; 2] {
    let mut mask = [0u64; 2];
    for character in value.bytes() {
        if character.is_ascii_lowercase() {
            let bit = (character - b'a') as usize;
            mask[bit / 64] |= 1u64 << (bit % 64);
        }
    }
    mask
}

fn is_app_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(extension)
            if extension.eq_ignore_ascii_case("exe")
                || extension.eq_ignore_ascii_case("lnk")
                || extension.eq_ignore_ascii_case("url")
    )
}

fn seed_windows_tools() -> Vec<IndexedEntry> {
    let system = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    let system32 = PathBuf::from(&system).join("System32");
    [
        ("Control Panel", system32.join("control.exe")),
        ("Task Manager", system32.join("Taskmgr.exe")),
        ("File Explorer", PathBuf::from(&system).join("explorer.exe")),
    ]
    .into_iter()
    .map(|(title, target)| IndexedEntry {
        title: title.to_string(),
        target: target.to_string_lossy().into_owned(),
        alternate_name: target
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string(),
        result_type: "App".to_string(),
        normalized_title: String::new(),
        normalized_alternate_name: String::new(),
        character_mask: [0, 0],
        base_score: 100,
        usage_boost: 0,
        last_launch: 0,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::SearchIndex;
    use std::fs;
    use std::path::PathBuf;

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
        fs::write(root.join("Google Chrome.url"), b"shortcut").unwrap();
        let persistence = root.join("index.json");

        let mut index = SearchIndex::default();
        index.refresh_roots(std::slice::from_ref(&root), &persistence);

        let results = index.search("chrome", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].target.ends_with("Google Chrome.url"));
        let reloaded = SearchIndex::load(&persistence);
        assert_eq!(reloaded.entries.len(), 4);
        let _ = fs::remove_dir_all(root);
    }
}
