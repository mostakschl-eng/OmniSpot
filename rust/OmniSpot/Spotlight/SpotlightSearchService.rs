use super::{SearchResult, SearchResultType, SpotlightUsageStore};
use crate::{
    configuration::Configuration,
    window_discovery::{IconExtractor, PackagedApps, WindowEnumerator},
};
use std::{collections::HashSet, path::Path};

pub struct SpotlightSearchService {
    usage: SpotlightUsageStore,
    packages: PackagedApps,
    icons: IconExtractor,
}
impl SpotlightSearchService {
    pub fn new(usage: SpotlightUsageStore) -> Self {
        Self {
            usage,
            packages: PackagedApps::default(),
            icons: IconExtractor,
        }
    }
    pub fn suggestions(
        &mut self,
        config: &Configuration,
        windows: &mut WindowEnumerator,
        recent_process_names: &[String],
    ) -> Vec<SearchResult> {
        let frequent = self.usage.get_frequent_targets(10);
        let catalog = self.catalog(config, windows);
        let mut results = frequent
            .iter()
            .filter_map(|target| {
                catalog
                    .iter()
                    .find(|result| result.target.eq_ignore_ascii_case(target))
                    .cloned()
            })
            .collect::<Vec<_>>();
        let mut catalog = catalog;
        catalog.sort_by_key(|result| {
            recent_process_names
                .iter()
                .position(|process| process.eq_ignore_ascii_case(&result.target))
                .unwrap_or(usize::MAX)
        });
        for result in catalog {
            if results.len() >= 10 {
                break;
            }
            if !results
                .iter()
                .any(|existing| existing.target.eq_ignore_ascii_case(&result.target))
            {
                results.push(result);
            }
        }
        results
    }
    pub fn search_apps(
        &mut self,
        query: &str,
        config: &Configuration,
        windows: &mut WindowEnumerator,
        recent_process_names: &[String],
    ) -> Vec<SearchResult> {
        if query.trim().is_empty() {
            return self.suggestions(config, windows, recent_process_names);
        }
        let query = normalize(query);
        let mut results = self
            .catalog(config, windows)
            .into_iter()
            .filter_map(|mut result| {
                let score = candidate_score(&result.title, &query).max(candidate_score(
                    Path::new(&result.target)
                        .file_stem()
                        .and_then(|name| name.to_str())
                        .unwrap_or(""),
                    &query,
                ));
                if score == 0 {
                    return None;
                }
                result.score += score + self.usage.get_boost(&result.target);
                Some(result)
            })
            .collect::<Vec<_>>();
        results.sort_by_key(|result| std::cmp::Reverse(result.score));
        results.truncate(25);
        results
    }
    pub fn record_launch(&self, result: &SearchResult) {
        if result.result_type == SearchResultType::App {
            let _ = self.usage.record_launch(&result.target);
        }
    }
    fn catalog(
        &mut self,
        config: &Configuration,
        windows: &mut WindowEnumerator,
    ) -> Vec<SearchResult> {
        let mut seen = HashSet::new();
        let mut results = Vec::new();
        for app in &config.applications {
            if seen.insert(app.process_path.to_ascii_lowercase()) {
                let mut result =
                    SearchResult::app(app.process_name(), &app.process_path, &app.process_path, 10);
                result.icon_path = self.icons.get_by_process_path(&app.process_path);
                results.push(result);
            }
        }
        for window in windows.get_cached_windows() {
            if seen.insert(window.process_image_path.to_ascii_lowercase()) {
                let mut result = SearchResult::app(
                    window.process_name(),
                    &window.process_image_path,
                    &window.process_image_path,
                    0,
                );
                result.icon_path = self.icons.get_by_process_path(&window.process_image_path);
                results.push(result);
            }
        }
        for app in self.packages.searchable() {
            let Some(aumid) = app.aumid else { continue };
            let target = format!("shell:AppsFolder\\{aumid}");
            if seen.insert(target.to_ascii_lowercase()) {
                if let Some(icon_path) = app.icon_path {
                    results.push(SearchResult::app_with_icon(
                        app.name,
                        app.installed_path,
                        target,
                        5,
                        icon_path,
                    ));
                } else {
                    results.push(SearchResult::app(app.name, app.installed_path, target, 5));
                }
            }
        }
        results
    }
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .split([' ', '\t', '_', '-', '.'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
fn candidate_score(candidate: &str, query: &str) -> i32 {
    let candidate = normalize(candidate);
    if candidate == query {
        return 100;
    }
    if candidate.starts_with(query) {
        return 90;
    }
    if candidate.split(' ').any(|token| token.starts_with(query)) {
        return 85;
    }
    if candidate.contains(query) {
        return 80;
    }
    if query.len() >= 3 && is_subsequence(query, &candidate) {
        return 45;
    }
    0
}
fn is_subsequence(query: &str, candidate: &str) -> bool {
    let mut iterator = candidate.chars();
    query
        .chars()
        .all(|character| iterator.any(|candidate_character| candidate_character == character))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranking_prefers_exact_name() {
        assert!(
            candidate_score("Visual Studio Code", "visual")
                > candidate_score("Code Viewer", "visual")
        );
    }
}
