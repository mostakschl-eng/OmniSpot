use super::{create_entry, IndexedEntry};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub(super) fn is_user_profile_root(path: &Path) -> bool {
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

pub(super) fn is_uninstaller_or_helper(name: &str) -> bool {
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

pub(super) fn is_indexable_file_ext(ext: &str) -> bool {
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

pub(super) fn scan_app_directory(
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

pub(super) fn scan_file_directory(
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
