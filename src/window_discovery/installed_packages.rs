use super::{ordinal_key, root_length, starts_with_ignore_case};
use std::collections::HashSet;

pub(crate) struct InstalledPackages {
    pub paths: Vec<String>,
    roots: Vec<String>,
    keys: HashSet<Vec<u16>>,
}

impl InstalledPackages {
    pub fn new(paths: &[String]) -> Self {
        let mut installed_paths = Vec::<String>::new();
        let mut roots = Vec::<String>::new();
        let mut keys = HashSet::new();
        let mut root_keys = HashSet::new();
        for path in paths {
            let root_length = root_length(path);
            let path = if path.len() > root_length && path.ends_with(['\\', '/']) {
                &path[..path.len() - 1]
            } else {
                path
            };
            if keys.insert(ordinal_key(path)) {
                installed_paths.push(path.to_string());
            }
            if path.len() > root_length {
                let mut end = path
                    .rfind(['\\', '/'])
                    .unwrap_or(root_length)
                    .max(root_length);
                while end > root_length
                    && matches!(path.as_bytes().get(end - 1), Some(b'\\' | b'/'))
                {
                    end -= 1;
                }
                let root = &path[..end];
                if root_keys.insert(ordinal_key(root)) {
                    roots.push(root.to_string());
                }
            }
        }
        Self {
            paths: installed_paths,
            roots,
            keys,
        }
    }

    pub fn contains(&self, process_path: &str) -> bool {
        self.roots.iter().any(|root| {
            if !starts_with_ignore_case(process_path, root) {
                return false;
            }
            let mut child_start = process_path
                .char_indices()
                .nth(root.chars().count())
                .map_or(process_path.len(), |(i, _)| i);
            if !root.ends_with(['\\', '/']) {
                if !matches!(process_path.as_bytes().get(child_start), Some(b'\\' | b'/')) {
                    return false;
                }
                child_start += 1;
            }
            let Some(rest) = process_path.get(child_start..) else {
                return false;
            };
            let Some(index) = rest.find(['\\', '/']) else {
                return false;
            };
            let package_directory = &process_path[..child_start + index];
            self.keys.contains(&ordinal_key(package_directory))
        })
    }
}
