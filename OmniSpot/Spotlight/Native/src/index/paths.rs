use super::scan::is_indexable_file_ext;
use crate::model::IndexedResult;
use std::fs;
use std::path::PathBuf;

pub(super) fn parse_extension_filter(query: &str) -> (Option<String>, String) {
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

pub(super) fn browse_directory(query: &str, max_results: usize) -> Option<Vec<IndexedResult>> {
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
