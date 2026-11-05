use super::{SearchResult, SearchResultType};
use crate::window_discovery::IconExtractor;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct FileSearchProvider {
    roots: Vec<PathBuf>,
    icons: IconExtractor,
}
impl Default for FileSearchProvider {
    fn default() -> Self {
        let roots = ["USERPROFILE", "APPDATA", "PROGRAMDATA"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(PathBuf::from)
            .collect();
        Self {
            roots,
            icons: IconExtractor::default(),
        }
    }
}
impl FileSearchProvider {
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            icons: IconExtractor::default(),
        }
    }
    pub fn search(&self, query: &str, max_results: usize) -> Vec<SearchResult> {
        let query = query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let terms = query.split_whitespace().collect::<Vec<_>>();
        let mut results = Vec::new();
        for root in &self.roots {
            self.visit(root, &terms, &mut results, max_results);
            if results.len() >= max_results {
                break;
            }
        }
        results.sort_by(|left, right| right.score.cmp(&left.score));
        results.truncate(max_results);
        results
    }
    fn visit(
        &self,
        path: &Path,
        terms: &[&str],
        results: &mut Vec<SearchResult>,
        max_results: usize,
    ) {
        if results.len() >= max_results {
            return;
        }
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            if results.len() >= max_results {
                return;
            }
            let path = entry.path();
            let name = match path.file_name().and_then(|name| name.to_str()) {
                Some(name) => name,
                None => continue,
            };
            if path.is_dir() {
                if !is_hidden_directory(name) {
                    self.visit(&path, terms, results, max_results);
                }
                continue;
            }
            if is_excluded_file(name) {
                continue;
            }
            let normalized = name.to_ascii_lowercase();
            if terms.iter().all(|term| normalized.contains(term)) {
                let score = if normalized.starts_with(terms[0]) {
                    90
                } else {
                    50
                };
                results.push(SearchResult {
                    title: name.to_string(),
                    subtitle: path.to_string_lossy().into_owned(),
                    target: path.to_string_lossy().into_owned(),
                    result_type: SearchResultType::File,
                    score,
                    icon_path: self.icons.get_by_path(&path),
                });
            }
        }
    }
}
fn is_hidden_directory(name: &str) -> bool {
    matches!(name, "AppData" | "node_modules" | ".git" | "target") || name.starts_with('.')
}
fn is_excluded_file(name: &str) -> bool {
    [".dll", ".exe", ".sys", ".pdb"]
        .iter()
        .any(|extension| name.to_ascii_lowercase().ends_with(extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn searches_files_and_ignores_executable_artifacts() {
        let root = std::env::temp_dir().join(format!("omnispot-search-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("notes.txt"), "test").unwrap();
        fs::write(root.join("notes.exe"), "test").unwrap();
        let results = FileSearchProvider::with_roots(vec![root.clone()]).search("notes", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result_type, SearchResultType::File);
        let _ = fs::remove_dir_all(root);
    }
}
