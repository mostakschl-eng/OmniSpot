use crate::model::Candidate;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::path::PathBuf;
use std::slice;
use std::sync::{Mutex, OnceLock};

static INDEX: OnceLock<Mutex<crate::index::SearchIndex>> = OnceLock::new();
static WATCHERS: OnceLock<Mutex<Vec<RecommendedWatcher>>> = OnceLock::new();

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
    let mut response = match serde_json::to_vec(value) {
        Ok(response) => response,
        Err(_) => return std::ptr::null_mut(),
    };
    let length = response.len();
    let pointer = response.as_mut_ptr();
    std::mem::forget(response);
    unsafe { *output_len = length; }
    pointer
}

#[no_mangle]
pub extern "C" fn omnispot_spotlight_initialize(
    config_ptr: *const u8,
    config_len: usize,
) -> u32 {
    if config_ptr.is_null() {
        return 0;
    }
    let config_bytes = unsafe { slice::from_raw_parts(config_ptr, config_len) };
    let config = match serde_json::from_slice::<IndexConfig>(config_bytes) {
        Ok(config) => config,
        Err(_) => return 0,
    };
    let index = INDEX.get_or_init(|| Mutex::new(crate::index::SearchIndex::load(
        std::path::Path::new(&config.persistence_path),
    )));
    let mut index = match index.lock() {
        Ok(index) => index,
        Err(_) => return 0,
    };
    let roots = config.roots.into_iter().map(PathBuf::from).collect::<Vec<_>>();
    index.refresh_roots(&roots, std::path::Path::new(&config.persistence_path));
    drop(index);

    let watchers = WATCHERS.get_or_init(|| Mutex::new(Vec::new()));
    let mut watchers = match watchers.lock() {
        Ok(watchers) => watchers,
        Err(_) => return 0,
    };
    for root in roots {
        let watcher_result = notify::recommended_watcher(|event: notify::Result<notify::Event>| {
            if event.is_ok() {
                if let Some(index) = INDEX.get().and_then(|index| index.lock().ok()) {
                    let mut index = index;
                    index.mark_dirty();
                }
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
    let index = match INDEX.get().and_then(|index| index.lock().ok()) {
        Some(index) => index,
        None => return std::ptr::null_mut(),
    };
    let mut index = index;
    index.refresh_if_dirty();
    write_json_response(&index.search(&request.query, request.max_results), output_len)
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
    let index = match INDEX.get().and_then(|index| index.lock().ok()) {
        Some(index) => index,
        None => return 0,
    };
    let mut index = index;
    index.record_launch(&request.target, std::path::Path::new(&request.persistence_path));
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
        Ok(request) => crate::rank_candidates(&request.query, &request.candidates, request.max_results),
        Err(_) => Vec::new(),
    };

    let mut bytes = match serde_json::to_vec(&response) {
        Ok(bytes) => bytes,
        Err(_) => return std::ptr::null_mut(),
    };
    let length = bytes.len();
    let pointer = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    unsafe { *output_len = length; }
    pointer
}

#[no_mangle]
pub extern "C" fn omnispot_spotlight_free(pointer: *mut u8, length: usize) {
    if pointer.is_null() {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(pointer, length, length));
    }
}

#[no_mangle]
pub extern "C" fn omnispot_spotlight_version() -> u32 {
    1
}

