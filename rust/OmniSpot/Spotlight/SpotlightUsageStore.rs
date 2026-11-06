use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf, sync::Mutex};

#[derive(Serialize, Deserialize, Default)]
struct UsageDocument {
    counts: HashMap<String, i32>,
}

pub struct SpotlightUsageStore {
    path: PathBuf,
    counts: Mutex<Option<HashMap<String, i32>>>,
}
impl SpotlightUsageStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            counts: Mutex::new(None),
        }
    }
    pub fn get_boost(&self, target: &str) -> i32 {
        let counts = self.loaded();
        counts
            .get(&target.to_ascii_uppercase())
            .copied()
            .unwrap_or(0)
            .min(6)
            * 2
    }
    pub fn get_frequent_targets(&self, max_count: usize) -> Vec<String> {
        let mut values = self.loaded().into_iter().collect::<Vec<_>>();
        values.sort_by_key(|value| std::cmp::Reverse(value.1));
        values
            .into_iter()
            .take(max_count)
            .map(|(target, _)| target)
            .collect()
    }
    pub fn record_launch(&self, target: &str) -> Result<(), String> {
        let mut guard = self
            .counts
            .lock()
            .map_err(|_| "usage lock poisoned".to_string())?;
        let counts = guard.get_or_insert_with(|| read_counts(&self.path));
        let key = target.to_ascii_uppercase();
        *counts.entry(key).or_default() += 1;
        let text = serde_json::to_string_pretty(&UsageDocument {
            counts: counts.clone(),
        })
        .map_err(|error| error.to_string())?;
        crate::atomic_file::write(&self.path, text.as_bytes()).map_err(|error| error.to_string())
    }
    fn loaded(&self) -> HashMap<String, i32> {
        self.counts
            .lock()
            .map(|mut guard| guard.get_or_insert_with(|| read_counts(&self.path)).clone())
            .unwrap_or_default()
    }
}
fn read_counts(path: &PathBuf) -> HashMap<String, i32> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<UsageDocument>(&text).ok())
        .map(|document| document.counts)
        .or_else(|| {
            path.parent()
                .and_then(|directory| {
                    crate::legacy_import::profile_bundle(directory)
                        .ok()
                        .flatten()
                })
                .map(|bundle| {
                    bundle
                        .spotlight_usage
                        .counts
                        .iter()
                        .map(|(key, count)| (key.clone(), *count))
                        .collect()
                })
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usage_boost_is_capped_and_case_insensitive() {
        let path = std::env::temp_dir().join(format!("omnispot-usage-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        let store = SpotlightUsageStore::new(&path);
        for _ in 0..10 {
            store.record_launch("notepad.exe").unwrap();
        }
        assert_eq!(store.get_boost("NOTEPAD.EXE"), 12);
        let _ = fs::remove_file(path);
    }
}
