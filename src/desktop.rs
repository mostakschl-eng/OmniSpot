use std::fmt::Write;
use std::path::Path;

pub use crate::configuration::app_theme_setting::AppThemeSetting;
pub use crate::configuration::application_type::ApplicationType;
pub use crate::configuration::configuration::{ApplicationConfiguration, Configuration, CycleMode};
pub use crate::desktop_hook::{CaptureEvent, DesktopCapture, DesktopInstance};
pub use crate::desktop_icons::{shell_icon, ICON_SIZE};
pub use crate::spotlight::model::IndexedResult;

pub fn load_settings(path: &Path) -> Result<Configuration, String> {
    let settings = crate::storage::load_desktop_settings(path.to_owned())
        .map_err(|error| error.to_string())?;
    let registry = crate::storage::read_desktop_registry(path.to_owned())
        .map_err(|error| error.to_string())?;
    crate::ffi::load_desktop_registry(registry);
    Ok(settings)
}

pub fn save_settings(path: &Path, configuration: Configuration) -> Result<(), String> {
    crate::storage::save_desktop_settings(path.to_owned(), configuration)
        .map_err(|error| error.to_string())
}

pub const MODIFIERS: [u32; 9] = crate::configuration::configuration_validator::VALID_MODIFIERS;

pub fn startup_enabled() -> bool {
    crate::startup::auto_start::AutoStart::new("OmniSpot").is_enabled()
}

pub fn set_startup(enabled: bool) -> Result<(), String> {
    let startup = crate::startup::auto_start::AutoStart::new("OmniSpot");
    let ok = if enabled {
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        startup.create_shortcut_with_args(&exe, "--run")
    } else {
        startup.remove_shortcut()
    };
    ok.then_some(())
        .ok_or_else(|| "Could not update the Windows startup shortcut.".into())
}

pub fn pulse_supported() -> bool {
    use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
    #[link(name = "ntdll")]
    extern "system" {
        fn RtlGetVersion(version: *mut OSVERSIONINFOW) -> i32;
    }
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    unsafe { RtlGetVersion(&mut version) == 0 && version.dwBuildNumber >= 22000 }
}

#[derive(Default)]
pub struct DesktopStats {
    pub cards: Vec<(String, String, Option<i32>)>,
    pub week: Vec<(String, i32)>,
}

pub fn stats_metrics(path: &Path, config: &Configuration) -> Result<DesktopStats, String> {
    use crate::stats::i_app_registry_cache::IAppRegistryCache;
    use crate::stats::stats_calculator::{add_days, StatsCalculator};
    use crate::stats::storage::DailyBucketDocument;

    struct DisplayNames;
    impl IAppRegistryCache for DisplayNames {
        fn get_display_name(&self, process_name: &str, process_path: Option<&str>) -> String {
            crate::ffi::desktop_display_name(process_name, process_path)
        }
    }
    let mut buckets = crate::storage::desktop_stats_buckets(path.to_owned())
        .map_err(|error| error.to_string())?;
    if crate::ffi::omnispot_stats_service_is_running() != 0 {
        if let Some(session) = crate::ffi::STATS_SESSION.get() {
            let session = session.lock().map_err(|_| "Stats session is unavailable")?;
            for snapshot in session.get_snapshots() {
                buckets.retain(|bucket| bucket.date != snapshot.date);
                buckets.push(snapshot);
            }
        }
    }
    buckets.retain(|bucket| {
        bucket.total_switches > 0 || bucket.total_peeks > 0 || bucket.alt_tab_switches > 0
    });
    if buckets.is_empty() {
        return Ok(DesktopStats::default());
    }
    let date = crate::stats::session_stats::system_today();
    let today = buckets
        .iter()
        .position(|bucket| bucket.date == date)
        .map(|index| buckets.remove(index))
        .unwrap_or_else(|| DailyBucketDocument::new(&date));
    buckets.retain(|bucket| bucket.date < date);
    let shortcuts = config
        .applications
        .iter()
        .filter_map(|app| {
            let index = app
                .key
                .checked_sub(FIRST_HOTKEY)
                .filter(|&index| index < 26)?;
            let key = char::from_u32('A' as u32 + index)?.to_string();
            Some((app.process_name().to_owned(), key))
        })
        .collect::<Vec<_>>();
    let metrics = StatsCalculator::new(DisplayNames).compute(&buckets, &today, &shortcuts);
    let week = (-6..=0)
        .map(|offset| {
            let date = add_days(&today.date, offset);
            let saved = buckets
                .iter()
                .chain(std::iter::once(&today))
                .find(|bucket| bucket.date == date)
                .map_or(0, |bucket| bucket.total_time_saved_ms);
            (date[5..].to_owned(), saved)
        })
        .collect();
    let mut cards = vec![
        ("Life gained back".into(), metrics.life_gained, None),
        (
            "Today’s progress".into(),
            format!("{} / 20 switches", metrics.today_switch_count),
            Some(metrics.today_switch_count.clamp(0, 20) * 5),
        ),
        (
            "Productivity streak".into(),
            format!("{} days", metrics.teleport_streak),
            None,
        ),
        (
            "Muscle Memory Index".into(),
            format!(
                "{} · {}",
                metrics.muscle_memo_grade, metrics.muscle_memo_persona
            ),
            metrics
                .is_session_warmup
                .then_some(metrics.total_switch_count.clamp(0, 10) * 10),
        ),
        (
            "Speed Demon".into(),
            format!(
                "{} switches · {} average latency",
                metrics.total_switch_count, metrics.avg_latency
            ),
            None,
        ),
        (
            "Peek Story".into(),
            format!(
                "{} peeks · {} average glance",
                metrics.total_peek_count, metrics.avg_glance_sec
            ),
            None,
        ),
        (
            "Habit Tracker".into(),
            format!(
                "{}% Alt-Tab relapses · {} day sober streak",
                metrics.alt_tab_relapse_pct, metrics.alt_tab_sober_streak
            ),
            Some(metrics.alt_tab_relapse_pct),
        ),
        (
            "Discovery Gap".into(),
            format!(
                "{}% static · {}% dynamic",
                metrics.static_pct,
                100 - metrics.static_pct
            ),
            Some(metrics.static_pct),
        ),
    ];
    if !metrics.podium.is_empty() {
        cards.push((
            "Most Used Apps".into(),
            metrics
                .podium
                .iter()
                .map(|app| format!("{} ({})", app.display_name, app.switches))
                .collect::<Vec<_>>()
                .join(" · "),
            None,
        ));
    }
    Ok(DesktopStats { cards, week })
}

pub fn clear_stats(path: &Path) -> Result<(), String> {
    if crate::ffi::omnispot_stats_service_is_running() != 0 {
        if crate::ffi::omnispot_stats_service_update(0) == 0 {
            return Err("Could not stop the stats worker.".into());
        }
        let reset = crate::storage::reset_running_desktop_stats(path.to_owned())
            .map_err(|error| error.to_string());
        let resumed = crate::ffi::omnispot_stats_service_update(1) != 0;
        reset?;
        return resumed
            .then_some(())
            .ok_or_else(|| "Stats were cleared. Could not restart collection.".into());
    }
    crate::storage::clear_desktop_stats(path.to_owned()).map_err(|error| error.to_string())
}

pub fn update_stats(path: &Path, enabled: bool) -> Result<(), String> {
    let running = crate::ffi::omnispot_stats_service_is_running() != 0;
    if enabled && !running {
        crate::storage::prepare_desktop_stats(path.to_owned())
            .map_err(|error| error.to_string())?;
    }
    if crate::ffi::omnispot_stats_service_update(u32::from(enabled)) == 0 {
        return Err("Could not update the stats worker.".into());
    }
    if running && !enabled {
        flush_stats(path)?;
    }
    Ok(())
}

pub fn flush_stats(path: &Path) -> Result<(), String> {
    crate::storage::flush_desktop_stats(path.to_owned()).map_err(|error| error.to_string())?;
    crate::storage::persist_desktop_registry(path.with_file_name("settings.sqlite3"))
        .map_err(|error| error.to_string())
}

pub fn import_stats(path: &Path) -> Result<(), String> {
    crate::storage::import_desktop_stats(path.to_owned()).map_err(|error| error.to_string())
}

pub fn default_profile_dir() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let parent = exe
        .parent()
        .ok_or("Could not locate the application directory")?;
    if parent.join(".portable").is_file() {
        return Ok(parent.to_owned());
    }
    let app_data =
        std::env::var_os("APPDATA").ok_or("Could not locate roaming application data")?;
    Ok(Path::new(&app_data).join("OmniSpot"))
}

pub fn set_passive_window(handle: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW,
    };
    let hwnd = HWND(handle as *mut _);
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let style = (style & !WS_EX_APPWINDOW.0) | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0;
        SetWindowLongW(hwnd, GWL_EXSTYLE, style as i32);
    }
}

pub fn set_window_icon(handle: isize) {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        LoadIconW, SendMessageW, ICON_BIG, ICON_SMALL, WM_SETICON,
    };
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            return;
        };
        let Ok(icon) = LoadIconW(module, PCWSTR(1 as *const u16)) else {
            return;
        };
        let window = HWND(handle as *mut _);
        for size in [ICON_BIG, ICON_SMALL] {
            SendMessageW(
                window,
                WM_SETICON,
                WPARAM(size as usize),
                LPARAM(icon.0 as isize),
            );
        }
    }
}

pub fn place_popup(width: u32, height: u32, scale: f32, spotlight: bool) -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor) }.ok()?;
    let mut monitor = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let display = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    if !unsafe { GetMonitorInfoW(display, &mut monitor) }.as_bool() {
        return None;
    }
    let area = monitor.rcWork;
    let (width, height) = (width as i32, height as i32);
    let gap = (16.0 * scale).round() as i32;
    let (x, y) = if spotlight {
        (
            area.left + (area.right - area.left - width) / 2,
            area.top + (area.bottom - area.top) / 5,
        )
    } else {
        let below = cursor.y + gap;
        (
            cursor.x - width / 2,
            if below + height > area.bottom {
                cursor.y - gap - height
            } else {
                below
            },
        )
    };
    Some((
        x.clamp(area.left, area.left.max(area.right - width)),
        y.clamp(area.top, area.top.max(area.bottom - height)),
    ))
}

pub fn focus_window(handle: isize) {
    use windows::Win32::Foundation::HWND;
    let _ = unsafe {
        windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(HWND(handle as *mut _))
    };
}

pub fn show_startup_error(message: &str) {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let message: Vec<u16> = message.encode_utf16().chain([0]).collect();
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            w!("OmniSpot"),
            MB_OK | MB_ICONERROR,
        )
    };
}

pub fn spotlight_index_path(profile: &Path, config: &Configuration) -> std::path::PathBuf {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hash = DefaultHasher::new();
    config.spotlight_custom_directories.hash(&mut hash);
    profile.join(format!("spotlight-index-{:016x}.json", hash.finish()))
}

pub struct SpotlightSearch {
    index: crate::spotlight::index::SearchIndex,
    path: std::path::PathBuf,
}

impl SpotlightSearch {
    pub fn load(configuration: &Configuration, path: std::path::PathBuf) -> Self {
        use crate::spotlight::index::SearchIndex;
        let mut index = SearchIndex::load(&path);
        if index.entries.is_empty() {
            let mut roots = Vec::new();
            if let Some(home) = std::env::var_os("USERPROFILE") {
                roots.push(home.into());
            }
            for name in ["APPDATA", "PROGRAMDATA"] {
                if let Some(base) = std::env::var_os(name) {
                    roots.push(
                        Path::new(&base)
                            .join("Microsoft")
                            .join("Windows")
                            .join("Start Menu"),
                    );
                }
            }
            roots.extend(
                configuration
                    .spotlight_custom_directories
                    .iter()
                    .flatten()
                    .map(std::path::PathBuf::from),
            );
            index = SearchIndex::scan_all(&roots, &[], &path);
            index.save(&path);
        }
        Self { index, path }
    }

    pub fn search(&self, query: &str) -> Vec<IndexedResult> {
        let query = query.trim();
        if query.is_empty() {
            self.index
                .search(query, 40)
                .into_iter()
                .filter(|item| item.result_type == "App")
                .take(10)
                .collect()
        } else {
            self.index.search(query, 16)
        }
    }

    pub fn launch(&mut self, target: &str, kind: &str) -> Result<(), String> {
        crate::spotlight::launch::launch_target(target, kind).map_err(|error| error.message)?;
        if kind == "App" {
            self.index.record_launch(target, &self.path);
        }
        Ok(())
    }
}

pub struct OverlayItem {
    pub section: String,
    pub label: String,
    pub badge: String,
    pub icon_path: String,
    pub active: bool,
    pub elevated: bool,
    target: OverlayTarget,
}

enum OverlayTarget {
    Application(ApplicationConfiguration),
    Window { handle: i64, process_id: u32 },
}

impl OverlayItem {
    pub fn activate(&self) -> Result<(), String> {
        use crate::window_discovery::window_enumerator::{get_windows, WindowEnumerator};
        let switcher = crate::switcher::Switcher::new(WindowEnumerator::default());
        match &self.target {
            OverlayTarget::Application(application) => switcher
                .open(application, "")
                .map_err(|error| error.message),
            OverlayTarget::Window { handle, process_id } => {
                let window = get_windows(true)
                    .into_iter()
                    .find(|window| window.handle == *handle && window.process_id == *process_id)
                    .ok_or("This window is no longer available")?;
                switcher.activate_window(&window);
                Ok(())
            }
        }
    }
}

fn discover_dynamic_apps(
    configuration: &Configuration,
    windows: &[crate::window_discovery::application_window::ApplicationWindow],
) -> Vec<ApplicationConfiguration> {
    use crate::input::dynamic_mode_service::DynamicModeService;
    use crate::stats::i_app_registry_cache::IAppRegistryCache;
    use crate::window_discovery::{
        app_name_resolver::AppNameResolver, window_enumerator::WindowEnumerator,
    };
    struct DisplayNames;
    impl IAppRegistryCache for DisplayNames {
        fn get_display_name(&self, process_name: &str, process_path: Option<&str>) -> String {
            crate::ffi::desktop_display_name(process_name, process_path)
        }
    }
    let service = DynamicModeService {
        window_enumerator: WindowEnumerator::default(),
        app_name_resolver: AppNameResolver {
            app_registry_cache: DisplayNames,
        },
        packaged_apps_service: Default::default(),
    };
    service
        .get_all_dynamic_apps(&configuration.applications, windows)
        .unwrap_or_default()
}

pub fn dynamic_applications(configuration: &Configuration) -> Vec<ApplicationConfiguration> {
    if !configuration.dynamic_mode_enabled {
        return Vec::new();
    }
    discover_dynamic_apps(
        configuration,
        &crate::window_discovery::window_enumerator::get_windows(true),
    )
}

pub fn overlay_items(configuration: &Configuration) -> Vec<OverlayItem> {
    use crate::window_discovery::{
        ends_with_ignore_case, process_inspector, window_enumerator::get_windows,
    };
    let windows = get_windows(true);
    let foreground =
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() }.0 as i64;
    let mut items = Vec::new();
    let mut groups = std::collections::HashSet::new();
    for app in &configuration.applications {
        let mut matches = windows
            .iter()
            .filter(|window| ends_with_ignore_case(&window.process_image_path, app.process_name()))
            .collect::<Vec<_>>();
        let display = process_inspector::get_process_display_name(&app.process_path);
        if app.cycle_mode == CycleMode::NextWindow
            && matches.len() > 1
            && groups.insert(app.process_name().to_lowercase())
        {
            matches.sort_unstable_by_key(|window| window.handle);
            matches.truncate(10);
            let titles = matches
                .iter()
                .map(|window| window.title.as_deref().unwrap_or_default())
                .collect::<Vec<_>>();
            let suffix = shared_title_suffix(&titles).unwrap_or_default();
            let icon_path = application_icon_path(app);
            for (index, window) in matches.iter().enumerate() {
                let title = titles[index]
                    .strip_suffix(suffix)
                    .unwrap_or(titles[index])
                    .trim_end();
                items.push(OverlayItem {
                    section: display.clone(),
                    label: title.into(),
                    badge: ((index + 1) % 10).to_string(),
                    icon_path: icon_path.clone(),
                    active: window.handle == foreground,
                    elevated: window.needs_elevation,
                    target: OverlayTarget::Window {
                        handle: window.handle,
                        process_id: window.process_id,
                    },
                });
            }
        }
        if !matches.is_empty() || app.start_if_not_running {
            let section = if matches.is_empty() {
                "Launchable apps"
            } else {
                "Running apps"
            };
            let active = matches.iter().any(|window| window.handle == foreground);
            let elevated = matches.iter().any(|window| window.needs_elevation);
            items.push(overlay_application(
                section,
                app.clone(),
                display,
                active,
                elevated,
            ));
        }
    }
    if configuration.dynamic_mode_enabled {
        items.extend(
            discover_dynamic_apps(configuration, &windows)
                .into_iter()
                .map(|app| {
                    let name = process_inspector::get_process_display_name(&app.process_path);
                    let matching = windows.iter().filter(|window| {
                        ends_with_ignore_case(&window.process_image_path, app.process_name())
                    });
                    let active = matching.clone().any(|window| window.handle == foreground);
                    let elevated = matching.into_iter().any(|window| window.needs_elevation);
                    overlay_application("Dynamic apps", app, name, active, elevated)
                }),
        );
    }
    items.sort_by_key(|item| match item.section.as_str() {
        "Running apps" => 1,
        "Launchable apps" => 2,
        "Dynamic apps" => 3,
        _ => 0,
    });
    items
}

fn overlay_application(
    section: &str,
    app: ApplicationConfiguration,
    display: String,
    active: bool,
    elevated: bool,
) -> OverlayItem {
    let key = app
        .key
        .checked_sub(FIRST_HOTKEY)
        .filter(|&key| key < 26)
        .and_then(|key| char::from_u32('A' as u32 + key));
    OverlayItem {
        section: section.into(),
        label: display,
        badge: key.map(|key| key.to_string()).unwrap_or_default(),
        icon_path: application_icon_path(&app),
        active,
        elevated,
        target: OverlayTarget::Application(app),
    }
}

pub fn application_icon_path(app: &ApplicationConfiguration) -> String {
    if app.application_type == ApplicationType::Packaged {
        let mut packages =
            crate::window_discovery::packaged_apps_service::PackagedAppsService::default();
        if let Some(path) = packages
            .get_by_aumid(app.aumid.as_deref())
            .ok()
            .flatten()
            .or_else(|| {
                packages
                    .get_by_installed_path(&app.process_path, None)
                    .ok()
                    .flatten()
            })
            .map(|info| info.icon_path)
            .filter(|path| !path.is_empty())
        {
            return path;
        }
    }
    app.process_path.clone()
}

pub fn application_display_name(path: &str) -> String {
    crate::ffi::desktop_display_name(crate::runtime::file_name(path), Some(path))
}

pub fn search_icon_path(result: &IndexedResult) -> String {
    if let Some(aumid) = result
        .target
        .rsplit('\\')
        .next()
        .filter(|value| value.contains('!'))
    {
        let mut packages =
            crate::window_discovery::packaged_apps_service::PackagedAppsService::default();
        if let Ok(Some(info)) = packages.get_by_aumid(Some(aumid)) {
            if !info.icon_path.is_empty() {
                return info.icon_path;
            }
        }
    }
    result.icon_path.clone()
}

fn shared_title_suffix<'a>(titles: &[&'a str]) -> Option<&'a str> {
    let [first, rest @ ..] = titles else {
        return None;
    };
    if rest.is_empty() {
        return None;
    }
    let suffix = first
        .char_indices()
        .map(|(index, _)| &first[index..])
        .find(|suffix| rest.iter().all(|title| title.ends_with(suffix)))?;
    [" - ", " — ", " | ", " : "]
        .into_iter()
        .any(|separator| {
            suffix
                .strip_prefix(separator)
                .is_some_and(|name| name.chars().count() >= 2)
        })
        .then_some(suffix)
}

pub const FIRST_HOTKEY: u32 = crate::input::keyboard_input::A;

pub fn is_executable(path: &str) -> bool {
    crate::window_discovery::process_inspector::is_windows_executable(path)
}

pub fn running_application_aumid(path: &str) -> Option<String> {
    let window = crate::window_discovery::window_enumerator::get_windows(true)
        .into_iter()
        .find(|window| {
            crate::window_discovery::equal_ignore_case(&window.process_image_path, path)
        })?;
    crate::window_discovery::packaged_apps_service::PackagedAppsService::default()
        .get_by_installed_path(path, Some(window.process_id))
        .ok()
        .flatten()
        .map(|package| package.aumid)
}

pub fn running_app_paths() -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut paths = crate::window_discovery::window_enumerator::get_windows(true)
        .into_iter()
        .map(|window| window.process_image_path)
        .filter(|path| !path.is_empty() && seen.insert(path.to_lowercase()))
        .collect::<Vec<_>>();
    paths.sort_unstable_by_key(|path| path.to_lowercase());
    paths
}

pub fn choose_path(directory: bool) -> Result<Option<String>, String> {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS,
        SIGDN_FILESYSPATH,
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|error| error.to_string())?;
        let _apartment = Apartment;
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| error.to_string())?;
        let options = dialog.GetOptions().map_err(|error| error.to_string())?;
        let kind = if directory {
            FOS_PICKFOLDERS
        } else {
            FOS_FILEMUSTEXIST
        };
        dialog
            .SetOptions(options | FOS_FORCEFILESYSTEM | kind)
            .map_err(|error| error.to_string())?;
        if let Err(error) = dialog.Show(windows::Win32::Foundation::HWND::default()) {
            if error.code().0 as u32 == 0x800704C7 {
                return Ok(None);
            }
            return Err(error.to_string());
        }
        let item = dialog.GetResult().map_err(|error| error.to_string())?;
        let path = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|error| error.to_string())?;
        let text = path.to_string();
        CoTaskMemFree(Some(path.0.cast()));
        text.map(Some).map_err(|error| error.to_string())
    }
}

pub fn open_in_shell(target: &str) -> Result<(), String> {
    crate::runtime::shell_open(target).map_err(|error| error.message)
}

pub fn open_log_folder() -> Result<(), String> {
    let folder = log_directory()?;
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let target = folder.to_str().ok_or("Log directory has an invalid path")?;
    open_in_shell(target)
}

pub fn log_window_diagnostics() -> Result<(), String> {
    let folder = log_directory()?;
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let (_, csv) = window_diagnostics();
    std::fs::write(folder.join("window-diagnostics.csv"), csv).map_err(|error| error.to_string())
}

fn log_directory() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let directory = exe
        .parent()
        .ok_or("Could not locate the application directory")?;
    Ok(if directory.join(".portable").is_file() {
        directory.join("logs")
    } else {
        let local =
            std::env::var_os("LOCALAPPDATA").ok_or("Could not locate local application data")?;
        Path::new(&local).join("OmniSpot").join("logs")
    })
}

pub fn window_diagnostics() -> (usize, String) {
    use crate::window_discovery::{process_inspector, window_enumerator};
    let windows = window_enumerator::get_windows(false);
    let mut csv = String::from(
        "ProcessId,Handle,ProcessImagePath,ProductName,Title,State,Style,StyleEx,IsCloaked\r\n",
    );
    for window in &windows {
        let product = process_inspector::get_product_name(&window.process_image_path)
            .ok()
            .flatten()
            .unwrap_or_default();
        let _ = write!(
            csv,
            "{},0x{:X},{},{},{},{},0x{:08X},0x{:08X},{}\r\n",
            window.process_id,
            window.handle,
            csv_field(&window.process_image_path),
            csv_field(&product),
            csv_field(window.title.as_deref().unwrap_or_default()),
            window.state,
            window.style,
            window.style_ex,
            window.is_cloaked,
        );
    }
    (windows.len(), csv)
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn overlay_title_suffix_matches_only_delimited_app_names() {
        assert_eq!(
            super::shared_title_suffix(&["Document — Editor", "Settings — Editor"]),
            Some(" — Editor")
        );
        assert_eq!(
            super::shared_title_suffix(&["Untitled One", "Page One"]),
            None
        );
        assert_eq!(super::shared_title_suffix(&["Only - Editor"]), None);
    }

    #[test]
    fn diagnostic_csv_quotes_window_titles_with_separators() {
        assert_eq!(super::csv_field("OmniSpot"), "OmniSpot");
        assert_eq!(super::csv_field("a,\"b\"\n"), "\"a,\"\"b\"\"\n\"");
    }

    #[test]
    fn desktop_settings_round_trip_and_legacy_profiles_are_preserved() {
        let directory =
            std::env::temp_dir().join(format!("omnispot-desktop-settings-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("settings.sqlite3");
        let legacy = directory.join("settings.db");
        std::fs::write(&legacy, b"old settings").unwrap();
        assert!(super::load_settings(&path).is_err());
        assert!(!path.exists());
        assert_eq!(std::fs::read(&legacy).unwrap(), b"old settings");
        std::fs::remove_file(legacy).unwrap();

        let mut config = super::load_settings(&path).unwrap();
        config.overlay_enabled = true;
        config.overlay_show_delay_ms = 450;
        config.theme = super::AppThemeSetting::Dark;
        super::save_settings(&path, config.clone()).unwrap();
        assert_eq!(super::load_settings(&path).unwrap(), config);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
