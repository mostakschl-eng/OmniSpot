use crate::model::{Candidate, ExtraAppItem};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::path::PathBuf;
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};

static INDEX: OnceLock<RwLock<crate::index::SearchIndex>> = OnceLock::new();
static WATCHERS: OnceLock<Mutex<Vec<RecommendedWatcher>>> = OnceLock::new();
static DIRTY_FLAG: AtomicBool = AtomicBool::new(false);
static WORKER_INIT: std::sync::Once = std::sync::Once::new();

fn start_background_worker() {
    WORKER_INIT.call_once(|| {
        std::thread::spawn(|| {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if DIRTY_FLAG.swap(false, Ordering::Relaxed) {
                    // Settle period: wait 4 seconds for file activity to finish (debounce)
                    std::thread::sleep(std::time::Duration::from_secs(4));
                    DIRTY_FLAG.store(false, Ordering::Relaxed);

                    if let Some(index_rw) = INDEX.get() {
                        let (roots, extra_apps, persistence) = match index_rw.read() {
                            Ok(guard) => (
                                guard.roots.clone(),
                                guard.extra_apps.clone(),
                                guard.persistence_path.clone(),
                            ),
                            Err(_) => continue,
                        };
                        if let Some(persistence_path) = persistence {
                            let mut new_index = crate::index::SearchIndex::scan_all(
                                &roots,
                                &extra_apps,
                                &persistence_path,
                            );
                            if let Ok(mut guard) = index_rw.write() {
                                if guard.roots != roots
                                    || guard.extra_apps != extra_apps
                                    || guard.persistence_path.as_deref()
                                        != Some(persistence_path.as_path())
                                {
                                    continue;
                                }
                                guard.swap_entries(&mut new_index);
                                guard.save(&persistence_path);
                            }
                        }
                    }
                }
            }
        });
    });
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    query: String,
    candidates: Vec<Candidate>,
    max_results: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexConfig {
    roots: Vec<String>,
    persistence_path: String,
    #[serde(default)]
    extra_apps: Option<Vec<ExtraAppItem>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexSearchRequest {
    query: String,
    max_results: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordLaunchRequest {
    target: String,
    persistence_path: String,
}

fn write_json_response<T: serde::Serialize>(value: &T, output_len: *mut usize) -> *mut u8 {
    let response = match serde_json::to_vec(value) {
        Ok(response) => response,
        Err(_) => return std::ptr::null_mut(),
    };
    let response = response.into_boxed_slice();
    let length = response.len();
    let pointer = Box::into_raw(response) as *mut u8;
    unsafe {
        *output_len = length;
    }
    pointer
}

#[no_mangle]
pub extern "C" fn omnispot_spotlight_initialize(config_ptr: *const u8, config_len: usize) -> u32 {
    if config_ptr.is_null() {
        return 0;
    }
    let config_bytes = unsafe { slice::from_raw_parts(config_ptr, config_len) };
    let config = match serde_json::from_slice::<IndexConfig>(config_bytes) {
        Ok(config) => config,
        Err(_) => return 0,
    };
    let watchers = WATCHERS.get_or_init(|| Mutex::new(Vec::new()));
    let mut watchers = match watchers.lock() {
        Ok(watchers) => watchers,
        Err(_) => return 0,
    };
    let index = INDEX.get_or_init(|| {
        RwLock::new(crate::index::SearchIndex::load(std::path::Path::new(
            &config.persistence_path,
        )))
    });
    let mut index = match index.write() {
        Ok(index) => index,
        Err(_) => return 0,
    };
    let extra_apps = config.extra_apps.unwrap_or_default();
    let roots = config
        .roots
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if index.roots == roots
        && index.extra_apps == extra_apps
        && index.persistence_path.as_deref() == Some(std::path::Path::new(&config.persistence_path))
    {
        return 1;
    }
    index.refresh_roots(
        &roots,
        &extra_apps,
        std::path::Path::new(&config.persistence_path),
    );
    drop(index);

    watchers.clear();
    for root in roots {
        let persistence = PathBuf::from(&config.persistence_path);
        let temporary = persistence.with_extension("tmp");
        let watcher_result =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if event.is_ok_and(|event| {
                    !matches!(event.kind, notify::EventKind::Access(_))
                        && event
                            .paths
                            .iter()
                            .any(|path| path != &persistence && path != &temporary)
                }) {
                    DIRTY_FLAG.store(true, Ordering::Relaxed);
                }
            });
        let mut watcher = match watcher_result {
            Ok(watcher) => watcher,
            Err(_) => continue,
        };
        if watcher.watch(&root, RecursiveMode::Recursive).is_ok() {
            watchers.push(watcher);
        }
    }
    start_background_worker();
    1
}

#[no_mangle]
pub extern "C" fn omnispot_spotlight_search_index(
    request_ptr: *const u8,
    request_len: usize,
    output_len: *mut usize,
) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() {
        return std::ptr::null_mut();
    }
    let request_bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let request = match serde_json::from_slice::<IndexSearchRequest>(request_bytes) {
        Ok(request) => request,
        Err(_) => return std::ptr::null_mut(),
    };
    let index_guard = match INDEX.get().and_then(|index| index.read().ok()) {
        Some(index) => index,
        None => return std::ptr::null_mut(),
    };
    let results = index_guard.search(&request.query, request.max_results);
    drop(index_guard);
    write_json_response(&results, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_spotlight_record_launch(
    request_ptr: *const u8,
    request_len: usize,
) -> u32 {
    if request_ptr.is_null() {
        return 0;
    }
    let request_bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let request = match serde_json::from_slice::<RecordLaunchRequest>(request_bytes) {
        Ok(request) => request,
        Err(_) => return 0,
    };
    let index = match INDEX.get().and_then(|index| index.write().ok()) {
        Some(index) => index,
        None => return 0,
    };
    let mut index = index;
    index.record_launch(
        &request.target,
        std::path::Path::new(&request.persistence_path),
    );
    1
}

#[no_mangle]
pub extern "C" fn omnispot_spotlight_search(
    request_ptr: *const u8,
    request_len: usize,
    output_len: *mut usize,
) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() {
        return std::ptr::null_mut();
    }

    let request_bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let response = match serde_json::from_slice::<Request>(request_bytes) {
        Ok(request) => {
            crate::rank_candidates(&request.query, &request.candidates, request.max_results)
        }
        Err(_) => Vec::new(),
    };

    write_json_response(&response, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_spotlight_free(pointer: *mut u8, length: usize) {
    if pointer.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            pointer, length,
        )));
    }
}

#[no_mangle]
pub extern "C" fn omnispot_spotlight_version() -> u32 {
    1
}

#[cfg(test)]
#[path = "tests/ffi_tests.rs"]
mod tests;
