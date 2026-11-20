use crate::spotlight::model::{Candidate, ExtraAppItem};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::path::PathBuf;
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};
use std::collections::VecDeque;

static INDEX: OnceLock<RwLock<crate::spotlight::index::SearchIndex>> = OnceLock::new();
static WATCHERS: OnceLock<Mutex<Vec<RecommendedWatcher>>> = OnceLock::new();
static DIRTY_FLAG: AtomicBool = AtomicBool::new(false);
static WORKER_INIT: std::sync::Once = std::sync::Once::new();
static SWITCHER: OnceLock<Mutex<crate::switcher::Switcher<crate::window_discovery::window_enumerator::WindowEnumerator>>> = OnceLock::new();
static WINDOW_ENUMERATOR: OnceLock<Mutex<crate::window_discovery::window_enumerator::WindowEnumerator>> = OnceLock::new();
pub(crate) static STATS_SESSION: OnceLock<Mutex<crate::stats::session_stats::SessionStats>> = OnceLock::new();
static PACKAGED_APPS: OnceLock<Mutex<crate::window_discovery::packaged_apps_service::PackagedAppsService>> = OnceLock::new();
static APP_REGISTRY: OnceLock<Mutex<std::collections::HashMap<String, String>>> = OnceLock::new();
static INSTALLED_PACKAGE_PATHS: OnceLock<Vec<String>> = OnceLock::new();
static CONFIGURATION_RUNTIME: OnceLock<Mutex<ConfigurationRuntimeState>> = OnceLock::new();
static STATS_RUNTIME: OnceLock<StatsRuntime> = OnceLock::new();
static STATS_CHANGED_CALLBACK: OnceLock<Mutex<Option<extern "C" fn()>>> = OnceLock::new();
static STATS_FLUSH_CALLBACK: OnceLock<Mutex<Option<extern "C" fn()>>> = OnceLock::new();
type NativeHook = crate::input::hook::Hook<
    crate::window_discovery::window_enumerator::WindowEnumerator,
    ManagedHookNameResolver,
    ManagedHookHost,
>;
type ManagedHookCallback = extern "C" fn(u32, *const u8, usize) -> u32;
static HOOK_CALLBACK: OnceLock<Mutex<Option<ManagedHookCallback>>> = OnceLock::new();
static HOOK_RUNTIME: OnceLock<Mutex<Option<NativeHook>>> = OnceLock::new();

struct ManagedHookNameResolver;
impl crate::window_discovery::i_app_name_resolver::IAppNameResolver for ManagedHookNameResolver {
    fn get_dynamic_key(&self, process_name: &str, process_path: &str) -> Option<u32> {
        let request = serde_json::json!({"processName": process_name, "processPath": process_path});
        let bytes = serde_json::to_vec(&request).ok()?;
        let key = call_hook_host(12, &bytes);
        if key == 0 { None } else { Some(key) }
    }
}

struct ManagedHookHost;
impl crate::input::hook::HookHost for ManagedHookHost {
    fn subscribe_keyboard(&mut self) -> Result<(), crate::runtime::RuntimeError> { Ok(()) }
    fn unsubscribe_keyboard(&mut self) {}
    fn dispose_keyboard(&mut self) {}
    fn configure_overlay_timer(&mut self, timeout_ms: i32) {
        if let Ok(bytes) = serde_json::to_vec(&serde_json::json!({"timeoutMs": timeout_ms})) { call_hook_host(1, &bytes); }
    }
    fn start_overlay_timer(&mut self) { call_hook_host(2, &[]); }
    fn cancel_overlay_timer(&mut self) { call_hook_host(3, &[]); }
    fn overlay_is_visible(&self) -> bool { call_hook_host(4, &[]) != 0 }
    fn show_overlay(&mut self, applications: &[crate::configuration::configuration::ApplicationConfiguration], dynamic: bool) {
        if let Ok(bytes) = serde_json::to_vec(&serde_json::json!({"applications": applications, "dynamic": dynamic})) { call_hook_host(5, &bytes); }
    }
    fn hide_overlay(&mut self) { call_hook_host(6, &[]); }
    fn show_warning(&mut self, warning: crate::input::hook::Warning) {
        let value = match warning { crate::input::hook::Warning::Elevated => "elevated", crate::input::hook::Warning::KeyEventsStealing => "keyEventsStealing" };
        if let Ok(bytes) = serde_json::to_vec(value) { call_hook_host(7, &bytes); }
    }
    fn toggle_spotlight(&mut self) { call_hook_host(8, &[]); }
    fn enqueue_stats(&mut self, event: crate::stats::stats_event::StatsEvent) {
        let payload = match event {
            crate::stats::stats_event::StatsEvent::SwitchEvent { process_name, process_id, process_path, total_choices, modifier_down_tick, letter_down_tick, previous_letter_up_tick, is_dynamic, trigger_key } => serde_json::json!({
                "kind": "switch", "processName": process_name, "processId": process_id, "processPath": process_path,
                "totalChoices": total_choices, "modifierDownTick": modifier_down_tick, "letterDownTick": letter_down_tick,
                "previousLetterUpTick": previous_letter_up_tick, "isDynamic": is_dynamic, "triggerKey": trigger_key
            }),
            crate::stats::stats_event::StatsEvent::PeekEvent { target_process_name, target_process_path, arm_tick, finish_tick, is_dynamic } => serde_json::json!({
                "kind": "peek", "targetProcessName": target_process_name, "targetProcessPath": target_process_path,
                "armTick": arm_tick, "finishTick": finish_tick, "isDynamic": is_dynamic
            }),
            crate::stats::stats_event::StatsEvent::AltTabEvent(nav_count) => serde_json::json!({ "kind": "altTab", "navCount": nav_count }),
        };
        if let Ok(bytes) = serde_json::to_vec(&payload) { call_hook_host(9, &bytes); }
    }
    fn monitor_potential_elevation(&mut self, process_path: &str) {
        if let Ok(bytes) = serde_json::to_vec(process_path) { call_hook_host(10, &bytes); }
    }
    fn log_error(&mut self, error: &crate::runtime::RuntimeError) {
        if let Ok(bytes) = serde_json::to_vec(&error.message) { call_hook_host(11, &bytes); }
    }
}

fn call_hook_host(event: u32, payload: &[u8]) -> u32 {
    let callback = HOOK_CALLBACK.get_or_init(|| Mutex::new(None)).lock().ok().and_then(|callback| *callback);
    callback.map_or(0, |callback| callback(event, payload.as_ptr(), payload.len()))
}

fn new_native_hook() -> NativeHook {
    let windows = crate::window_discovery::window_enumerator::WindowEnumerator::default();
    let switcher = crate::switcher::Switcher::new(crate::window_discovery::window_enumerator::WindowEnumerator::default());
    let dynamic_mode = crate::input::dynamic_mode_service::DynamicModeService {
        window_enumerator: crate::window_discovery::window_enumerator::WindowEnumerator::default(),
        app_name_resolver: ManagedHookNameResolver,
        packaged_apps_service: crate::window_discovery::packaged_apps_service::PackagedAppsService::default(),
    };
    crate::input::hook::Hook::new(switcher, crate::input::peeker::Peeker::default(), windows, dynamic_mode, ManagedHookHost)
}

struct StatsRuntime {
    state: Mutex<StatsQueueState>,
    wake: Condvar,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[derive(Default)]
struct ConfigurationRuntimeState {
    current: Option<crate::configuration::configuration::Configuration>,
    migrations_run: bool,
}

struct StatsQueueState {
    queue: VecDeque<QueuedStatsEvent>,
    running: bool,
    stopping: bool,
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum QueuedStatsEvent {
    Switch {
        process_name: String,
        process_path: String,
        total_choices: i32,
        modifier_down_tick: i64,
        letter_down_tick: i64,
        previous_letter_up_tick: Option<i64>,
        is_dynamic: bool,
        trigger_key: String,
    },
    Peek {
        process_name: String,
        process_path: String,
        arm_tick: i64,
        finish_tick: i64,
        is_dynamic: bool,
    },
    AltTab { nav_count: i32 },
}

impl Default for StatsQueueState {
    fn default() -> Self {
        Self { queue: VecDeque::with_capacity(1000), running: false, stopping: false }
    }
}

pub(crate) fn enqueue_desktop_stats(event: crate::stats::stats_event::StatsEvent) {
    use crate::stats::stats_event::StatsEvent;
    let Some(runtime) = STATS_RUNTIME.get() else { return };
    let event = match event {
        StatsEvent::SwitchEvent { process_name, process_path, total_choices, modifier_down_tick,
            letter_down_tick, previous_letter_up_tick, is_dynamic, trigger_key, .. } => {
            let key = crate::input::keyboard_input::virtual_key_from_key(trigger_key);
            let trigger_key = char::from_u32(u32::from(key)).filter(|key| key.is_ascii_alphanumeric())
                .map(|key| key.to_string()).unwrap_or_default();
            QueuedStatsEvent::Switch { process_name, process_path, total_choices, modifier_down_tick,
                letter_down_tick, previous_letter_up_tick, is_dynamic, trigger_key }
        }
        StatsEvent::PeekEvent { target_process_name, target_process_path, arm_tick, finish_tick, is_dynamic } =>
            QueuedStatsEvent::Peek { process_name: target_process_name, process_path: target_process_path,
                arm_tick, finish_tick, is_dynamic },
        StatsEvent::AltTabEvent(nav_count) => QueuedStatsEvent::AltTab { nav_count },
    };
    let _ = runtime.enqueue(event);
}

pub(crate) fn desktop_display_name(process_name: &str, process_path: Option<&str>) -> String {
    resolve_app_registry_native(process_name, process_path).display_name
}

pub(crate) fn load_desktop_registry(entries: std::collections::HashMap<String, String>) {
    let registry = APP_REGISTRY.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Ok(mut registry) = registry.lock() {
        registry.extend(entries);
    }
}

pub(crate) fn desktop_registry() -> std::collections::HashMap<String, String> {
    APP_REGISTRY.get().and_then(|registry| registry.lock().ok())
        .map(|entries| entries.clone()).unwrap_or_default()
}

impl StatsRuntime {
    fn start(&self) -> bool {
        let Ok(mut state) = self.state.lock() else { return false; };
        if state.running { return true; }
        state.running = true;
        state.stopping = false;
        drop(state);
        let Ok(mut worker) = self.worker.lock() else { return false; };
        *worker = Some(std::thread::spawn(stats_worker));
        true
    }

    fn enqueue(&self, event: QueuedStatsEvent) -> bool {
        let Ok(mut state) = self.state.lock() else { return false; };
        if !state.running || state.stopping { return false; }
        if state.queue.len() == 1000 { state.queue.pop_front(); }
        state.queue.push_back(event);
        self.wake.notify_one();
        true
    }

    fn stop(&self) -> bool {
        if let Ok(mut state) = self.state.lock() {
            if !state.running { return true; }
            state.stopping = true;
            self.wake.notify_all();
        } else { return false; }
        if let Ok(mut worker) = self.worker.lock() {
            if let Some(handle) = worker.take() { let _ = handle.join(); }
        } else { return false; }
        if let Ok(mut state) = self.state.lock() {
            state.running = false;
            state.stopping = false;
            true
        } else { false }
    }
}

fn stats_worker() {
    let mut last_flush = Instant::now();
    loop {
        let runtime = STATS_RUNTIME.get().expect("stats runtime initialized");
        let (event, flush, stop) = {
            let Ok(state) = runtime.state.lock() else { return; };
            let timeout = Duration::from_secs(300).saturating_sub(last_flush.elapsed());
            let (mut state, timeout_result) = match runtime.wake.wait_timeout_while(
                state, timeout, |s| s.queue.is_empty() && !s.stopping,
            ) {
                Ok(result) => result,
                Err(_) => return,
            };
            if let Some(event) = state.queue.pop_front() { (Some(event), false, false) }
            else if state.stopping { (None, false, true) }
            else if timeout_result.timed_out() { (None, true, false) }
            else { continue; }
        };
        if stop { break; }
        if let Some(event) = event { process_queued_stats_event(event); }
        if flush {
            if let Some(callback) = STATS_FLUSH_CALLBACK.get().and_then(|slot| slot.lock().ok().and_then(|value| *value)) {
                callback();
            }
            last_flush = Instant::now();
        }
    }
}

fn process_queued_stats_event(event: QueuedStatsEvent) {
    let session = STATS_SESSION.get_or_init(|| Mutex::new(crate::stats::session_stats::SessionStats::new()));
    let Ok(session) = session.lock() else { return; };
    match event {
        QueuedStatsEvent::Switch { process_name, process_path, total_choices, modifier_down_tick,
            letter_down_tick, previous_letter_up_tick, is_dynamic, trigger_key } => {
            let _ = resolve_app_registry_native(&process_name, Some(&process_path));
            let start = previous_letter_up_tick.unwrap_or(modifier_down_tick);
            let raw = stats_elapsed_ms(start, letter_down_tick);
            let duration = if raw > 1500 { 350 } else { raw };
            let fastest = if raw <= 1500 { Some(raw) } else { None };
            let saved = crate::stats::efficiency_calculator::saved_ms(total_choices, duration);
            session.record_switch(&process_name, None, duration, saved, is_dynamic, fastest, &trigger_key);
        }
        QueuedStatsEvent::Peek { process_name, process_path, arm_tick, finish_tick, is_dynamic } => {
            let _ = resolve_app_registry_native(&process_name, Some(&process_path));
            session.record_peek(&process_name, stats_elapsed_ms(arm_tick, finish_tick), is_dynamic);
        }
        QueuedStatsEvent::AltTab { nav_count } => session.record_alt_tab(nav_count),
    }
    drop(session);
    if let Some(callback) = STATS_CHANGED_CALLBACK.get().and_then(|slot| slot.lock().ok().and_then(|value| *value)) {
        callback();
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatsSwitchRequest {
    process_name: String,
    total_choices: i32,
    modifier_down_tick: i64,
    letter_down_tick: i64,
    previous_letter_up_tick: Option<i64>,
    is_dynamic: bool,
    trigger_key: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatsPeekRequest {
    process_name: String,
    arm_tick: i64,
    finish_tick: i64,
    is_dynamic: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatsCalculateRequest {
    all_buckets: Vec<crate::stats::storage::DailyBucketDocument>,
    today: crate::stats::storage::DailyBucketDocument,
    configured_static_apps: Vec<ConfiguredStaticApp>,
    display_names: std::collections::HashMap<String, String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatsHistoricRequest {
    buckets: Vec<crate::stats::storage::DailyBucketDocument>,
    today: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfiguredStaticApp {
    process_name: String,
    key: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackagedAppsRequest {
    operation: String,
    #[serde(default)] path: Option<String>,
    #[serde(default)] process_id: Option<u32>,
    #[serde(default)] aumid: Option<String>,
    #[serde(default)] family_name: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunningAppsRequest {
    windows: Vec<crate::window_discovery::application_window::ApplicationWindow>,
    excluded_process_names: Vec<String>,
    packages: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProcessInspectorRequest {
    operation: String,
    #[serde(default)] path: Option<String>,
    #[serde(default)] process_id: Option<u32>,
    #[serde(default)] timeout_ms: Option<i32>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppRegistryRequest {
    operation: String,
    #[serde(default)] entries: Vec<AppRegistryEntry>,
    #[serde(default)] process_name: Option<String>,
    #[serde(default)] process_path: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppRegistryEntry {
    process_name: String,
    display_name: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppRegistryResponse {
    display_name: String,
    was_cached: bool,
    should_persist: bool,
}

struct StatsDisplayNames(std::collections::HashMap<String, String>);

impl crate::stats::i_app_registry_cache::IAppRegistryCache for StatsDisplayNames {
    fn get_display_name(&self, process_name: &str, _process_path: Option<&str>) -> String {
        self.0.iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(process_name))
            .map(|(_, display)| display.clone())
            .unwrap_or_else(|| std::path::Path::new(process_name).file_stem()
                .and_then(|stem| stem.to_str()).unwrap_or(process_name).to_string())
    }
}

fn stats_elapsed_ms(start: i64, end: i64) -> i32 {
    #[cfg(windows)]
    let frequency = {
        let mut value = 0i64;
        unsafe { let _ = windows::Win32::System::Performance::QueryPerformanceFrequency(&mut value); }
        value.max(1)
    };
    #[cfg(not(windows))]
    let frequency = 10_000_000i64;
    (((end - start).max(0) as f64 * 1000.0) / frequency as f64) as i32
}

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
                            let mut new_index = crate::spotlight::index::SearchIndex::scan_all(
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpotlightLaunchRequest {
    target: String,
    result_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CliParseRequest {
    args: Vec<String>,
    commands: Vec<String>,
    flags: Vec<String>,
    valued_options: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwitchRequest {
    applications: Vec<crate::configuration::configuration::ApplicationConfiguration>,
    culture: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwitchIndexRequest {
    applications: Vec<crate::configuration::configuration::ApplicationConfiguration>,
    index: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwitchOpenRequest {
    application: crate::configuration::configuration::ApplicationConfiguration,
    culture: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SwitchTargetRequest {
    target: String,
    needs_elevation: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SwitchResponse<T: serde::Serialize> {
    ok: bool,
    value: Option<T>,
    error: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SwitchIndexResponse {
    switched: bool,
    window: Option<crate::window_discovery::application_window::ApplicationWindow>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FocusedWindowsRequest {
    windows: Vec<crate::window_discovery::application_window::ApplicationWindow>,
    applications: Vec<crate::configuration::configuration::ApplicationConfiguration>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DynamicAppsRequest {
    windows: Vec<crate::window_discovery::application_window::ApplicationWindow>,
    applications: Vec<crate::configuration::configuration::ApplicationConfiguration>,
    resolved_keys: Vec<ResolvedDynamicKey>,
    #[serde(default)]
    installed_packages: Vec<String>,
    key: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedDynamicKey {
    process_path: String,
    key: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigSanitizeRequest {
    configuration: crate::configuration::configuration::Configuration,
    family_matches: Vec<FamilyMatch>,
    installed_paths: Vec<AumidPath>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FamilyMatch {
    family_name: String,
    aumid: String,
    icon_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AumidPath {
    aumid: String,
    path: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationResponse {
    status: &'static str,
    process: Option<String>,
    message: Option<String>,
}

struct NativePackagedAppsProvider {
    by_family: std::collections::HashMap<String, crate::configuration::packaged_app_path_sanitizer::PackagedAppInfo>,
    by_aumid: std::collections::HashMap<String, String>,
}

impl crate::configuration::packaged_app_path_sanitizer::IPackagedAppsService for NativePackagedAppsProvider {
    fn get_by_package_family_name(&self, family_name: &str) -> Option<crate::configuration::packaged_app_path_sanitizer::PackagedAppInfo> {
        self.by_family.get(family_name).cloned()
    }
    fn get_installed_path_by_aumid(&self, aumid: &str) -> Option<String> {
        self.by_aumid.get(aumid).cloned()
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CliParseResponse {
    status: &'static str,
    command: Option<String>,
    debug: bool,
    trace: bool,
    error: Option<String>,
    matched_flags: Vec<String>,
    unknown_arguments: Vec<String>,
    valued_values: Vec<(String, String)>,
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
pub extern "C" fn omnispot_storage_execute(
    request: *const u8,
    length: usize,
    output_len: *mut usize,
) -> *mut u8 {
    if request.is_null() || output_len.is_null() {
        return std::ptr::null_mut();
    }
    let request = unsafe { slice::from_raw_parts(request, length) };
    let response = match crate::storage::execute(request) {
        Ok(value) => serde_json::json!({ "value": value }),
        Err(error) => serde_json::json!({ "error": error.to_string() }),
    };
    write_json_response(&response, output_len)
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
        RwLock::new(crate::spotlight::index::SearchIndex::load(std::path::Path::new(
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
pub extern "C" fn omnispot_spotlight_launch_target(request_ptr: *const u8, request_len: usize) -> u32 {
    if request_ptr.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<SpotlightLaunchRequest>(bytes) else { return 0 };
    u32::from(crate::spotlight::launch::launch_target(&request.target, &request.result_type).is_ok())
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
            crate::spotlight::matcher::rank_candidates(&request.query, &request.candidates, request.max_results)
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

// Startup shortcut operations are kept in Rust so the C# shell can remain a UI host.
#[no_mangle]
pub extern "C" fn omnispot_autostart_is_enabled() -> u32 {
    let name = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_stem().map(|name| name.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "OmniSpot".to_string());
    u32::from(crate::startup::AutoStart::new(name).is_enabled())
}

#[no_mangle]
pub extern "C" fn omnispot_autostart_remove() -> u32 {
    let name = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_stem().map(|name| name.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "OmniSpot".to_string());
    u32::from(crate::startup::AutoStart::new(name).remove_shortcut())
}

#[no_mangle]
pub extern "C" fn omnispot_autostart_create() -> u32 {
    let Ok(executable) = std::env::current_exe() else { return 0 };
    let name = executable
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "OmniSpot".to_string());
    u32::from(crate::startup::AutoStart::new(name).create_shortcut(&executable))
}

#[no_mangle]
pub extern "C" fn omnispot_cli_parse(
    request_ptr: *const u8,
    request_len: usize,
    output_len: *mut usize,
) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() {
        return std::ptr::null_mut();
    }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<CliParseRequest>(bytes) else {
        return std::ptr::null_mut();
    };
    let mut debug = false;
    let mut trace = false;
    let mut commands = Vec::new();
    let mut matched_flags = Vec::new();
    let mut unknown_arguments = Vec::new();
    let mut valued_values = Vec::new();
    let mut i = 0;
    while i < request.args.len() {
        let arg = &request.args[i];
        if arg == "--help" {
            return write_json_response(&CliParseResponse { status: "help", command: None, debug, trace, error: None, matched_flags, valued_values, unknown_arguments }, output_len);
        }
        if request.flags.iter().any(|name| name == arg) {
            debug |= arg == "--debug";
            trace |= arg == "--trace";
            matched_flags.push(arg.clone());
            i += 1;
        } else if request.valued_options.iter().any(|name| name == arg) {
            if i + 1 == request.args.len() {
                return write_json_response(&CliParseResponse { status: "error", command: None, debug, trace, error: Some(format!("Option {arg} requires a value.")), matched_flags, valued_values, unknown_arguments }, output_len);
            }
            valued_values.push((arg.clone(), request.args[i + 1].clone()));
            i += 2;
        } else if request.commands.iter().any(|name| name == arg) {
            commands.push(arg.clone());
            i += 1;
        } else {
            unknown_arguments.push(arg.clone());
            i += 1;
        }
    }
    if commands.len() > 1 {
        return write_json_response(&CliParseResponse { status: "error", command: None, debug, trace, error: Some(format!("Multiple commands specified: {}\nOnly one command is allowed at a time.", commands.join(", "))), matched_flags, valued_values, unknown_arguments }, output_len);
    }
    write_json_response(&CliParseResponse {
        status: if request.args.is_empty() { "noArgs" } else if commands.is_empty() { "continue" } else { "command" },
        command: commands.into_iter().next(), debug, trace, error: None, matched_flags, valued_values, unknown_arguments,
    }, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_hook_set_callback(callback: Option<ManagedHookCallback>) -> u32 {
    let slot = HOOK_CALLBACK.get_or_init(|| Mutex::new(None));
    let Ok(mut slot) = slot.lock() else { return 0; };
    *slot = callback;
    1
}

#[no_mangle]
pub extern "C" fn omnispot_hook_start(request_ptr: *const u8, request_len: usize) -> u32 {
    if request_ptr.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(config) = serde_json::from_slice::<crate::configuration::configuration::Configuration>(bytes) else { return 0; };
    let slot = HOOK_RUNTIME.get_or_init(|| Mutex::new(None));
    let Ok(mut slot) = slot.lock() else { return 0; };
    let hook = slot.get_or_insert_with(new_native_hook);
    if hook.start(config).is_ok() { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn omnispot_hook_update_configuration(request_ptr: *const u8, request_len: usize) -> u32 {
    if request_ptr.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(config) = serde_json::from_slice::<crate::configuration::configuration::Configuration>(bytes) else { return 0; };
    let slot = HOOK_RUNTIME.get_or_init(|| Mutex::new(None));
    let Ok(mut slot) = slot.lock() else { return 0; };
    let hook = slot.get_or_insert_with(new_native_hook);
    hook.update_configuration(config);
    1
}

#[no_mangle]
pub extern "C" fn omnispot_hook_stop() -> u32 {
    let slot = HOOK_RUNTIME.get_or_init(|| Mutex::new(None));
    let Ok(mut slot) = slot.lock() else { return 0; };
    if let Some(hook) = slot.as_mut() { hook.stop(); }
    1
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HookEventRequest { key: u32, is_key_down: bool, flags: u32, culture: String }

#[no_mangle]
pub extern "C" fn omnispot_hook_keyboard_event(request_ptr: *const u8, request_len: usize, suppress: *mut u32) -> u32 {
    if request_ptr.is_null() || suppress.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<HookEventRequest>(bytes) else { return 0; };
    let slot = HOOK_RUNTIME.get_or_init(|| Mutex::new(None));
    let Ok(mut slot) = slot.lock() else { return 0; };
    let Some(hook) = slot.as_mut() else { return 0; };
    let mut event = crate::input::hook::KeyboardHookEvent { key: request.key, is_key_down: request.is_key_down, flags: request.flags, suppress_key_press: false };
    hook.keyboard_pressed(&mut event, &request.culture);
    unsafe { *suppress = u32::from(event.suppress_key_press); }
    1
}

#[no_mangle]
pub extern "C" fn omnispot_hook_set_overlay_preview(enabled: u32) -> u32 {
    let slot = HOOK_RUNTIME.get_or_init(|| Mutex::new(None));
    let Ok(mut slot) = slot.lock() else { return 0; };
    let Some(hook) = slot.as_mut() else { return 0; };
    hook.set_overlay_preview_enabled(enabled != 0);
    1
}

#[no_mangle]
pub extern "C" fn omnispot_hook_show_overlay_if_enabled() -> u32 {
    let slot = HOOK_RUNTIME.get_or_init(|| Mutex::new(None));
    let Ok(mut slot) = slot.lock() else { return 0; };
    let Some(hook) = slot.as_mut() else { return 0; };
    hook.show_overlay_if_enabled();
    1
}

#[no_mangle]
pub extern "C" fn omnispot_hook_potential_elevation_detected() -> u32 {
    let slot = HOOK_RUNTIME.get_or_init(|| Mutex::new(None));
    let Ok(mut slot) = slot.lock() else { return 0; };
    let Some(hook) = slot.as_mut() else { return 0; };
    hook.potential_elevation_detected();
    1
}

#[no_mangle]
pub extern "C" fn omnispot_switcher_execute(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<SwitchRequest>(bytes) else { return std::ptr::null_mut() };
    let switcher = SWITCHER.get_or_init(|| Mutex::new(crate::switcher::Switcher::new(crate::window_discovery::window_enumerator::WindowEnumerator::default())));
    let response = match switcher.lock() {
        Ok(mut switcher) => match switcher.execute(&request.applications, &request.culture) {
            Ok(value) => SwitchResponse { ok: true, value, error: None },
            Err(error) => SwitchResponse::<crate::switcher::AppSwitchResult> { ok: false, value: None, error: Some(error.message) },
        },
        Err(_) => SwitchResponse::<crate::switcher::AppSwitchResult> { ok: false, value: None, error: Some("Switcher lock is unavailable".into()) },
    };
    write_json_response(&response, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_switcher_window_index(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<SwitchIndexRequest>(bytes) else { return std::ptr::null_mut() };
    let switcher = SWITCHER.get_or_init(|| Mutex::new(crate::switcher::Switcher::new(crate::window_discovery::window_enumerator::WindowEnumerator::default())));
    let response = match switcher.lock() {
        Ok(switcher) => match switcher.switch_to_window_by_index(&request.applications, request.index) {
            Ok((switched, window)) => SwitchResponse { ok: true, value: Some(SwitchIndexResponse { switched, window }), error: None },
            Err(error) => SwitchResponse::<SwitchIndexResponse> { ok: false, value: None, error: Some(error.message) },
        },
        Err(_) => SwitchResponse::<SwitchIndexResponse> { ok: false, value: None, error: Some("Switcher lock is unavailable".into()) },
    };
    write_json_response(&response, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_switcher_open(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<SwitchOpenRequest>(bytes) else { return std::ptr::null_mut() };
    let switcher = SWITCHER.get_or_init(|| Mutex::new(crate::switcher::Switcher::new(crate::window_discovery::window_enumerator::WindowEnumerator::default())));
    let response = match switcher.lock() {
        Ok(switcher) => match switcher.open(&request.application, &request.culture) {
            Ok(()) => SwitchResponse { ok: true, value: Some(true), error: None },
            Err(error) => SwitchResponse::<bool> { ok: false, value: None, error: Some(error.message) },
        },
        Err(_) => SwitchResponse::<bool> { ok: false, value: None, error: Some("Switcher lock is unavailable".into()) },
    };
    write_json_response(&response, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_switcher_activate(request_ptr: *const u8, request_len: usize) -> u32 {
    if request_ptr.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(target) = serde_json::from_slice::<SwitchTargetRequest>(bytes) else { return 0 };
    let Ok(handle) = target.target.parse::<i64>() else { return 0 };
    let window = crate::window_discovery::application_window::ApplicationWindow { handle, needs_elevation: target.needs_elevation, ..Default::default() };
    let switcher = SWITCHER.get_or_init(|| Mutex::new(crate::switcher::Switcher::new(crate::window_discovery::window_enumerator::WindowEnumerator::default())));
    if let Ok(switcher) = switcher.lock() { switcher.activate_window(&window); 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn omnispot_switcher_hide(handle: i64) -> u32 {
    let switcher = SWITCHER.get_or_init(|| Mutex::new(crate::switcher::Switcher::new(crate::window_discovery::window_enumerator::WindowEnumerator::default())));
    if let Ok(switcher) = switcher.lock() { switcher.hide_window(handle); 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn omnispot_windows_get_all(output_len: *mut usize) -> *mut u8 {
    if output_len.is_null() { return std::ptr::null_mut(); }
    write_json_response(&crate::window_discovery::window_enumerator::get_windows(true), output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_windows_get_all_including_invalid(output_len: *mut usize) -> *mut u8 {
    if output_len.is_null() { return std::ptr::null_mut(); }
    write_json_response(&crate::window_discovery::window_enumerator::get_windows(false), output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_windows_get_cached(output_len: *mut usize) -> *mut u8 {
    if output_len.is_null() { return std::ptr::null_mut(); }
    let enumerator = WINDOW_ENUMERATOR.get_or_init(|| Mutex::new(crate::window_discovery::window_enumerator::WindowEnumerator::default()));
    let Ok(mut enumerator) = enumerator.lock() else { return std::ptr::null_mut() };
    write_json_response(&enumerator.get_cached_windows(), output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_windows_get_current(output_len: *mut usize) -> *mut u8 {
    if output_len.is_null() { return std::ptr::null_mut(); }
    write_json_response(&crate::window_discovery::window_enumerator::get_current_window(), output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_windows_get_focused(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<FocusedWindowsRequest>(bytes) else { return std::ptr::null_mut() };
    let current = crate::window_discovery::window_enumerator::get_current_window();
    let result = crate::window_discovery::window_enumerator::get_focused_app_windows(current, request.windows, &request.applications);
    write_json_response(&result, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_dynamic_get_apps(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<DynamicAppsRequest>(bytes) else { return std::ptr::null_mut() };
    let resolved: std::collections::HashMap<_, _> = request.resolved_keys.into_iter()
        .map(|item| (crate::window_discovery::ordinal_key(&item.process_path), item.key)).collect();
    let excluded: std::collections::HashSet<_> = request.applications.iter()
        .map(|app| crate::window_discovery::ordinal_key(app.process_name()))
        .chain([crate::window_discovery::ordinal_key("omnispot.exe")]).collect();
    let static_keys: std::collections::HashSet<_> = request.applications.iter().map(|app| app.key).collect();
    let installed = crate::window_discovery::installed_packages::InstalledPackages::new(&request.installed_packages);
    let mut seen = std::collections::HashSet::new();
    let apps: Vec<_> = request.windows.iter().filter_map(|window| {
        if !seen.insert(crate::window_discovery::ordinal_key(&window.process_image_path))
            || excluded.contains(&crate::window_discovery::ordinal_key(window.process_name())) { return None; }
        let key = resolved.get(&crate::window_discovery::ordinal_key(&window.process_image_path)).copied().flatten()?;
        if request.key.is_some_and(|requested| requested != key) || static_keys.contains(&key) { return None; }
        let mut app = crate::configuration::configuration::ApplicationConfiguration::new(
            key, window.process_image_path.clone(), crate::configuration::configuration::CycleMode::NextApp, false);
        if request.key.is_none() && installed.contains(&window.process_image_path) {
            app.application_type = crate::configuration::application_type::ApplicationType::Packaged;
        }
        Some(app)
    }).collect();
    write_json_response(&apps, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_dynamic_resolve_key(request_ptr: *const u8, request_len: usize) -> u32 {
    if request_ptr.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(display_name) = serde_json::from_slice::<String>(bytes) else { return 0; };
    crate::window_discovery::app_name_resolver::get_dynamic_key(&display_name)
        .map(|offset| crate::input::keyboard_input::A + u32::from(offset))
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn omnispot_packaged_apps(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<PackagedAppsRequest>(bytes) else { return std::ptr::null_mut(); };
    let service = PACKAGED_APPS.get_or_init(|| Mutex::new(crate::window_discovery::packaged_apps_service::PackagedAppsService::default()));
    let Ok(mut service) = service.lock() else { return std::ptr::null_mut(); };
    match request.operation.as_str() {
        "installedPackages" => match service.get_installed_packages() {
            Ok(value) => write_json_response(&value, output_len), Err(_) => std::ptr::null_mut(),
        },
        "searchableApps" => match service.get_searchable_apps() {
            Ok(value) => write_json_response(&value, output_len), Err(_) => std::ptr::null_mut(),
        },
        "displayName" => match service.get_display_name(request.path.as_deref().unwrap_or_default()) {
            Ok(value) => write_json_response(&value, output_len), Err(_) => std::ptr::null_mut(),
        },
        "byInstalledPath" => match service.get_by_installed_path(request.path.as_deref().unwrap_or_default(), request.process_id) {
            Ok(value) => write_json_response(&value, output_len), Err(_) => std::ptr::null_mut(),
        },
        "installedPathByAumid" => match service.get_installed_path_by_aumid(request.aumid.as_deref()) {
            Ok(value) => write_json_response(&value, output_len), Err(_) => std::ptr::null_mut(),
        },
        "byAumid" => match service.get_by_aumid(request.aumid.as_deref()) {
            Ok(value) => write_json_response(&value, output_len), Err(_) => std::ptr::null_mut(),
        },
        "byFamilyName" => match service.get_by_package_family_name(request.family_name.as_deref().unwrap_or_default()) {
            Ok(value) => write_json_response(&value, output_len), Err(_) => std::ptr::null_mut(),
        },
        _ => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn omnispot_running_apps(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<RunningAppsRequest>(bytes) else { return std::ptr::null_mut(); };
    let result = crate::window_discovery::running_applications_service::get_running_applications(
        &request.windows, &request.excluded_process_names, &request.packages);
    write_json_response(&result, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_process_inspector(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<ProcessInspectorRequest>(bytes) else { return std::ptr::null_mut(); };
    let path = request.path.as_deref().unwrap_or_default();
    match request.operation.as_str() {
        "needsElevation" => write_json_response(&crate::window_discovery::process_inspector::needs_elevation(request.process_id.unwrap_or_default()), output_len),
        "waitForPotentialElevation" => match crate::window_discovery::process_inspector::wait_for_potential_elevation(path, request.timeout_ms.unwrap_or(2000)) {
            Ok(value) => write_json_response(&value, output_len), Err(_) => std::ptr::null_mut(),
        },
        "isWindowsExecutable" => write_json_response(&crate::window_discovery::process_inspector::is_windows_executable(path), output_len),
        "displayName" => write_json_response(&crate::window_discovery::process_inspector::get_process_display_name(path), output_len),
        _ => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn omnispot_configuration_validate(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(config) = serde_json::from_slice::<crate::configuration::configuration::Configuration>(bytes) else { return std::ptr::null_mut() };
    let result = crate::configuration::configuration_validator::ConfigurationValidator::validate(&config);
    let response = ValidationResponse {
        status: if result.status == crate::configuration::configuration_validator::ValidationResultStatus::Success { "success" } else { "error" },
        process: result.process,
        message: result.message,
    };
    write_json_response(&response, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_configuration_defaults(output_len: *mut usize) -> *mut u8 {
    if output_len.is_null() { return std::ptr::null_mut(); }
    write_json_response(&crate::configuration::configuration::Configuration::default(), output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_configuration_manager_get(output_len: *mut usize) -> *mut u8 {
    if output_len.is_null() { return std::ptr::null_mut(); }
    let runtime = CONFIGURATION_RUNTIME.get_or_init(|| Mutex::new(ConfigurationRuntimeState::default()));
    let Ok(runtime) = runtime.lock() else { return std::ptr::null_mut(); };
    write_json_response(&runtime.current, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_configuration_manager_set(request_ptr: *const u8, request_len: usize) -> u32 {
    if request_ptr.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(config) = serde_json::from_slice::<crate::configuration::configuration::Configuration>(bytes) else { return 0; };
    let runtime = CONFIGURATION_RUNTIME.get_or_init(|| Mutex::new(ConfigurationRuntimeState::default()));
    let Ok(mut runtime) = runtime.lock() else { return 0; };
    runtime.current = Some(config);
    1
}

#[no_mangle]
pub extern "C" fn omnispot_configuration_manager_migrations_run() -> u32 {
    let runtime = CONFIGURATION_RUNTIME.get_or_init(|| Mutex::new(ConfigurationRuntimeState::default()));
    runtime.lock().map(|runtime| u32::from(runtime.migrations_run)).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn omnispot_configuration_manager_mark_migrations_run() -> u32 {
    let runtime = CONFIGURATION_RUNTIME.get_or_init(|| Mutex::new(ConfigurationRuntimeState::default()));
    let Ok(mut runtime) = runtime.lock() else { return 0; };
    runtime.migrations_run = true;
    1
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigurationMigrationRequest {
    applications: Vec<crate::configuration::configuration::ApplicationConfiguration>,
    packaged_paths: Vec<PackagedPathMatch>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackagedPathMatch { process_path: String, aumid: String }

#[no_mangle]
pub extern "C" fn omnispot_configuration_migrate_packaged_apps(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<ConfigurationMigrationRequest>(bytes) else { return std::ptr::null_mut(); };
    let matches: std::collections::HashMap<String, String> = request.packaged_paths.into_iter()
        .map(|entry| (entry.process_path.to_lowercase(), entry.aumid)).collect();
    let applications: Vec<_> = request.applications.into_iter().map(|mut app| {
        if app.application_type == crate::configuration::application_type::ApplicationType::Win32 {
            if let Some(aumid) = matches.get(&app.process_path.to_lowercase()) {
                app.application_type = crate::configuration::application_type::ApplicationType::Packaged;
                app.aumid = Some(aumid.clone());
            }
        }
        app
    }).collect();
    write_json_response(&applications, output_len)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrationVersionsRequest { available_versions: Vec<i32>, executed_versions: Vec<i32> }

#[no_mangle]
pub extern "C" fn omnispot_migrations_pending_versions(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<MigrationVersionsRequest>(bytes) else { return std::ptr::null_mut(); };
    let executed: std::collections::HashSet<_> = request.executed_versions.into_iter().collect();
    let pending: std::collections::BTreeSet<_> = request.available_versions.into_iter()
        .filter(|version| !executed.contains(version)).collect();
    write_json_response(&pending.into_iter().collect::<Vec<_>>(), output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_stats_is_default_bucket_key(request_ptr: *const u8, request_len: usize) -> u32 {
    if request_ptr.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(key) = serde_json::from_slice::<String>(bytes) else { return 0; };
    u32::from(crate::stats::migrations::remove_default_date_bucket_migration::is_default_date_bucket(&key))
}

#[no_mangle]
pub extern "C" fn omnispot_configuration_validate_application(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(app) = serde_json::from_slice::<crate::configuration::configuration::ApplicationConfiguration>(bytes) else { return std::ptr::null_mut() };
    let result = crate::configuration::configuration_validator::ConfigurationValidator::validate_application(&app);
    let response = ValidationResponse {
        status: if result.status == crate::configuration::configuration_validator::ValidationResultStatus::Success { "success" } else { "error" },
        process: result.process,
        message: result.message,
    };
    write_json_response(&response, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_configuration_sanitize(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<ConfigSanitizeRequest>(bytes) else { return std::ptr::null_mut() };
    let provider = NativePackagedAppsProvider {
        by_family: request.family_matches.into_iter().map(|entry| (entry.family_name,
            crate::configuration::packaged_app_path_sanitizer::PackagedAppInfo { aumid: entry.aumid, icon_path: entry.icon_path })).collect(),
        by_aumid: request.installed_paths.into_iter().map(|entry| (entry.aumid, entry.path)).collect(),
    };
    let sanitizer = crate::configuration::packaged_app_path_sanitizer::PackagedAppPathSanitizer::new(provider);
    write_json_response(&sanitizer.sanitize(&request.configuration), output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_configuration_package_family(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(path) = serde_json::from_slice::<String>(bytes) else { return std::ptr::null_mut() };
    write_json_response(&crate::configuration::packaged_app_path_sanitizer::get_package_family_name(&path), output_len)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalDateKeyRequest { utc_ticks: i64, offset_ticks: i64 }

#[no_mangle]
pub extern "C" fn omnispot_stats_local_date_key(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<LocalDateKeyRequest>(bytes) else { return std::ptr::null_mut(); };
    let ticks = (request.utc_ticks as i128 + request.offset_ticks as i128).clamp(0, 3_155_378_975_999_999_999i128);
    let mut days = (ticks / 864_000_000_000i128) as u32;
    let mut year = 1u32 + (days / 146_097) * 400;
    days %= 146_097;
    while days >= if is_leap_year(year) { 366 } else { 365 } {
        days -= if is_leap_year(year) { 366 } else { 365 };
        year += 1;
    }
    let month_days = [31u32, if is_leap_year(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u32;
    for month_length in month_days {
        if days < month_length { break; }
        days -= month_length;
        month += 1;
    }
    write_json_response(&format!("{year:04}-{month:02}-{:02}", days + 1), output_len)
}

fn is_leap_year(year: u32) -> bool { year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) }

#[no_mangle]
pub extern "C" fn omnispot_stats_process_switch(request_ptr: *const u8, request_len: usize) -> u32 {
    if request_ptr.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(event) = serde_json::from_slice::<StatsSwitchRequest>(bytes) else { return 0; };
    let raw = stats_elapsed_ms(event.previous_letter_up_tick.unwrap_or(event.modifier_down_tick), event.letter_down_tick);
    let duration = if raw > 1500 { 350 } else { raw };
    let fastest = if raw <= 1500 { Some(raw) } else { None };
    let session = STATS_SESSION.get_or_init(|| Mutex::new(crate::stats::session_stats::SessionStats::new()));
    let Ok(session) = session.lock() else { return 0; };
    session.record_switch(
        &event.process_name,
        None,
        duration,
        crate::stats::efficiency_calculator::saved_ms(event.total_choices, duration),
        event.is_dynamic,
        fastest,
        &event.trigger_key,
    );
    1
}

#[no_mangle]
pub extern "C" fn omnispot_stats_process_peek(request_ptr: *const u8, request_len: usize) -> u32 {
    if request_ptr.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(event) = serde_json::from_slice::<StatsPeekRequest>(bytes) else { return 0; };
    let duration = stats_elapsed_ms(event.arm_tick, event.finish_tick);
    let session = STATS_SESSION.get_or_init(|| Mutex::new(crate::stats::session_stats::SessionStats::new()));
    let Ok(session) = session.lock() else { return 0; };
    session.record_peek(&event.process_name, duration, event.is_dynamic);
    1
}

#[no_mangle]
pub extern "C" fn omnispot_stats_process_alt_tab(nav_count: i32) -> u32 {
    let session = STATS_SESSION.get_or_init(|| Mutex::new(crate::stats::session_stats::SessionStats::new()));
    let Ok(session) = session.lock() else { return 0; };
    session.record_alt_tab(nav_count);
    1
}

#[no_mangle]
pub extern "C" fn omnispot_stats_get_snapshots(output_len: *mut usize) -> *mut u8 {
    if output_len.is_null() { return std::ptr::null_mut(); }
    let session = STATS_SESSION.get_or_init(|| Mutex::new(crate::stats::session_stats::SessionStats::new()));
    let Ok(session) = session.lock() else { return std::ptr::null_mut(); };
    write_json_response(&session.get_snapshots(), output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_stats_load_today(request_ptr: *const u8, request_len: usize) -> u32 {
    if request_ptr.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(document) = serde_json::from_slice::<crate::stats::storage::DailyBucketDocument>(bytes) else { return 0; };
    let session = STATS_SESSION.get_or_init(|| Mutex::new(crate::stats::session_stats::SessionStats::new()));
    let Ok(session) = session.lock() else { return 0; };
    session.load_from(&document);
    1
}

#[no_mangle]
pub extern "C" fn omnispot_stats_clear_rollover() -> u32 {
    let session = STATS_SESSION.get_or_init(|| Mutex::new(crate::stats::session_stats::SessionStats::new()));
    let Ok(session) = session.lock() else { return 0; };
    session.clear_rollover();
    1
}

#[no_mangle]
pub extern "C" fn omnispot_stats_clear_session() -> u32 {
    let session = STATS_SESSION.get_or_init(|| Mutex::new(crate::stats::session_stats::SessionStats::new()));
    let Ok(session) = session.lock() else { return 0; };
    session.clear();
    1
}

#[no_mangle]
pub extern "C" fn omnispot_stats_service_update(enabled: u32) -> u32 {
    let runtime = STATS_RUNTIME.get_or_init(|| StatsRuntime {
        state: Mutex::new(StatsQueueState::default()),
        wake: Condvar::new(),
        worker: Mutex::new(None),
    });
    if enabled != 0 { return u32::from(runtime.start()); }
    let was_running = runtime.state.lock().map(|state| state.running).unwrap_or(false);
    if !runtime.stop() { return 0; }
    if was_running { 2 } else { 1 }
}

#[no_mangle]
pub extern "C" fn omnispot_stats_service_is_running() -> u32 {
    STATS_RUNTIME.get().is_some_and(|runtime| runtime.state.lock().map(|state| state.running).unwrap_or(false)).into()
}

#[no_mangle]
pub extern "C" fn omnispot_stats_set_changed_callback(callback: Option<extern "C" fn()>) -> u32 {
    let slot = STATS_CHANGED_CALLBACK.get_or_init(|| Mutex::new(None));
    let Ok(mut current) = slot.lock() else { return 0; };
    *current = callback;
    1
}

#[no_mangle]
pub extern "C" fn omnispot_stats_set_flush_callback(callback: Option<extern "C" fn()>) -> u32 {
    let slot = STATS_FLUSH_CALLBACK.get_or_init(|| Mutex::new(None));
    let Ok(mut current) = slot.lock() else { return 0; };
    *current = callback;
    1
}

#[no_mangle]
pub extern "C" fn omnispot_stats_enqueue(request_ptr: *const u8, request_len: usize) -> u32 {
    if request_ptr.is_null() { return 0; }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(event) = serde_json::from_slice::<QueuedStatsEvent>(bytes) else { return 2; };
    if STATS_RUNTIME.get().is_some_and(|runtime| runtime.enqueue(event)) { 1 } else { 3 }
}

#[no_mangle]
pub extern "C" fn omnispot_stats_calculate(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<StatsCalculateRequest>(bytes) else { return std::ptr::null_mut(); };
    let configured: Vec<(String, String)> = request.configured_static_apps.into_iter()
        .map(|app| (app.process_name, app.key)).collect();
    let calculator = crate::stats::stats_calculator::StatsCalculator::new(StatsDisplayNames(request.display_names));
    let metrics = calculator.compute(&request.all_buckets, &request.today, &configured);
    write_json_response(&metrics, output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_stats_historic_buckets(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<StatsHistoricRequest>(bytes) else { return std::ptr::null_mut(); };
    let mut db = crate::stats::storage::StatsDatabase::default();
    for bucket in request.buckets {
        if let Ok(value) = serde_json::to_value(&bucket) {
            db.daily_buckets.insert(bucket.date.clone(), value);
        }
    }
    let repository = crate::stats::stats_repository::StatsRepository::new(std::sync::Arc::new(Mutex::new(db)));
    write_json_response(&repository.get_all_historic_buckets(Some(&request.today)), output_len)
}

#[no_mangle]
pub extern "C" fn omnispot_app_registry(request_ptr: *const u8, request_len: usize, output_len: *mut usize) -> *mut u8 {
    if request_ptr.is_null() || output_len.is_null() { return std::ptr::null_mut(); }
    let bytes = unsafe { slice::from_raw_parts(request_ptr, request_len) };
    let Ok(request) = serde_json::from_slice::<AppRegistryRequest>(bytes) else { return std::ptr::null_mut(); };
    let registry = APP_REGISTRY.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if request.operation == "initialize" {
        let Ok(mut registry) = registry.lock() else { return std::ptr::null_mut(); };
        registry.clear();
        for entry in request.entries {
            registry.insert(entry.process_name.to_lowercase(), entry.display_name);
        }
        return write_json_response(&true, output_len);
    }
    if request.operation == "entries" {
        let Ok(registry) = registry.lock() else { return std::ptr::null_mut(); };
        let entries: Vec<AppRegistryEntry> = registry.iter().map(|(process_name, display_name)| AppRegistryEntry {
            process_name: process_name.clone(), display_name: display_name.clone(),
        }).collect();
        return write_json_response(&entries, output_len);
    }
    if request.operation != "resolve" { return std::ptr::null_mut(); }
    let process_name = request.process_name.as_deref().unwrap_or_default();
    write_json_response(&resolve_app_registry_native(process_name, request.process_path.as_deref()), output_len)
}

fn resolve_app_registry_native(process_name: &str, process_path: Option<&str>) -> AppRegistryResponse {
    let registry = APP_REGISTRY.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let key = process_name.to_lowercase();
    let Ok(cache) = registry.lock() else {
        return AppRegistryResponse { display_name: process_name.to_string(), was_cached: false, should_persist: false };
    };
    if let Some(display_name) = cache.get(&key) {
        return AppRegistryResponse {
            display_name: display_name.clone(), was_cached: true, should_persist: false,
        };
    }
    drop(cache);

    let mut display_name = None;
    if let Some(path) = process_path.filter(|path| !path.is_empty()) {
        let installed_paths = INSTALLED_PACKAGE_PATHS.get_or_init(|| {
            let packages = PACKAGED_APPS.get_or_init(|| Mutex::new(crate::window_discovery::packaged_apps_service::PackagedAppsService::default()));
            packages.lock().ok().and_then(|packages| packages.get_installed_packages().ok()).unwrap_or_default()
        });
        let installed = crate::window_discovery::installed_packages::InstalledPackages::new(installed_paths);
        if installed.contains(path) {
            let packages = PACKAGED_APPS.get_or_init(|| Mutex::new(crate::window_discovery::packaged_apps_service::PackagedAppsService::default()));
            if let Ok(packages) = packages.lock() {
                display_name = packages.get_display_name(path).ok().flatten().filter(|name| !name.is_empty());
            }
        }
        if display_name.is_none() {
            display_name = Some(crate::window_discovery::process_inspector::get_process_display_name(path));
        }
    }
    let display_name = display_name.unwrap_or_else(|| {
        std::path::Path::new(process_name).file_stem().and_then(|name| name.to_str()).unwrap_or(process_name).to_string()
    });
    let should_persist = process_path.is_some();
    if should_persist {
        let Ok(mut registry) = registry.lock() else {
            return AppRegistryResponse { display_name, was_cached: false, should_persist: false };
        };
        if let Some(cached) = registry.get(&key) {
            return AppRegistryResponse {
                display_name: cached.clone(), was_cached: true, should_persist: false,
            };
        }
        registry.insert(key, display_name.clone());
    }
    AppRegistryResponse {
        display_name, was_cached: false, should_persist,
    }
}

#[cfg(test)]
#[path = "tests/ffi_tests.rs"]
mod tests;
