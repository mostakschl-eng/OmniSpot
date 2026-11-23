use crate::window_discovery::{
    equal_ignore_case, packaged_apps_service::get_aumid_by_process_id,
    window_enumerator::get_windows,
};
use std::{ffi::c_void, path::Path};
use windows::core::{IUnknown, IUnknown_Vtbl, Interface, GUID, HRESULT, HSTRING, PWSTR};
use windows::Win32::{
    Foundation::{HWND, RPC_E_CHANGED_MODE, S_OK},
    System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IPersistFile,
        StructuredStorage::PropVariantToStringAlloc, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    },
    UI::{
        Shell::{
            IEnumFullIDList, IShellItem2, IShellLinkW,
            PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow, PROPERTYKEY},
            SHCreateItemFromIDList, SHCreateItemFromParsingName, ShellLink,
            SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY,
        },
        WindowsAndMessaging::{
            GetForegroundWindow, GetWindow, GetWindowThreadProcessId, IsIconic,
            SetForegroundWindow, ShowWindowAsync, SwitchToThisWindow, GW_OWNER, SW_RESTORE,
            WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
        },
    },
};

const APP_USER_MODEL_ID: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
    pid: 5,
};
const LINK_TARGET: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xb9b4b3fc_2b51_4a42_b5d8_324146afcf25),
    pid: 2,
};
const TASKBAND_PIN: GUID = GUID::from_u128(0x90aa3a4e_1cba_4233_b8bb_535773d48449);
const PINNED_LIST_IID: GUID = GUID::from_u128(0x0dd79ae2_d156_45d4_9eeb_3b549769e940);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DockWindow {
    pub handle: i64,
    pub process_id: u32,
    pub title: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DockApp {
    pub id: String,
    pub name: String,
    pub icon_path: String,
    pub icon_index: Option<i32>,
    pub launch_target: Option<String>,
    pub windows: Vec<DockWindow>,
    pub active: bool,
    app_id: Option<String>,
    executable: String,
    distinct_shortcut: bool,
}

pub fn snapshot(previous_foreground: isize) -> Vec<DockApp> {
    let apartment = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let result = snapshot_in_apartment(
        previous_foreground,
        apartment.is_ok() || apartment == RPC_E_CHANGED_MODE,
    );
    if apartment.is_ok() {
        unsafe { CoUninitialize() };
    }
    result
}

fn snapshot_in_apartment(previous_foreground: isize, can_use_shell: bool) -> Vec<DockApp> {
    let foreground = if previous_foreground == 0 {
        unsafe { GetForegroundWindow() }.0 as isize
    } else {
        previous_foreground
    };
    let mut apps = if can_use_shell {
        shell_pins().unwrap_or_else(shortcut_pins)
    } else {
        Vec::new()
    };
    for window in get_windows(true) {
        if window.process_id == std::process::id() {
            continue;
        }
        let handle = HWND(window.handle as isize as *mut c_void);
        if window.style_ex & WS_EX_APPWINDOW.0 == 0
            && (window.style_ex & WS_EX_TOOLWINDOW.0 != 0
                || unsafe { GetWindow(handle, GW_OWNER) }.is_ok())
        {
            continue;
        }
        let app_id = can_use_shell
            .then(|| window_app_id(handle))
            .flatten()
            .or_else(|| get_aumid_by_process_id(window.process_id));
        let title = window.title.unwrap_or_default();
        let name = if window.process_image_path.is_empty()
            || crate::runtime::file_name(&window.process_image_path)
                .eq_ignore_ascii_case("ApplicationFrameHost.exe")
        {
            title.clone()
        } else {
            crate::desktop::application_display_name(&window.process_image_path)
        };
        merge_running_window(
            &mut apps,
            app_id,
            window.process_image_path,
            name,
            DockWindow {
                handle: window.handle,
                process_id: window.process_id,
                title,
            },
            window.handle == foreground as i64,
        );
    }
    apps
}

fn merge_running_window(
    apps: &mut Vec<DockApp>,
    app_id: Option<String>,
    executable: String,
    name: String,
    window: DockWindow,
    active: bool,
) {
    let same_id = |app: &DockApp| matches!((&app.app_id, &app_id), (Some(a), Some(b)) if equal_ignore_case(a, b));
    let executable_matches = |app: &DockApp| {
        !app.distinct_shortcut
            && !executable.is_empty()
            && equal_ignore_case(&app.executable, &executable)
    };
    let index = apps
        .iter()
        .position(same_id)
        .or_else(|| {
            unique_app(apps, |app| {
                app.launch_target.is_some()
                    && executable_matches(app)
                    && !has_distinct_app_id(app.app_id.as_deref())
                    && !has_distinct_app_id(app_id.as_deref())
            })
        })
        .or_else(|| {
            unique_app(apps, |app| {
                executable_matches(app)
                    && (app.app_id.is_none() || app_id.is_none())
                    && !has_distinct_app_id(app.app_id.as_deref())
                    && !has_distinct_app_id(app_id.as_deref())
            })
        });
    if let Some(index) = index {
        let app = &mut apps[index];
        app.windows.push(window);
        app.active |= active;
        if app.icon_path.is_empty() {
            app.icon_path = executable;
        }
        if app.app_id.is_none() {
            app.app_id = app_id;
        }
    } else {
        apps.push(DockApp {
            id: format!("window:{}", window.handle),
            name,
            icon_path: executable.clone(),
            executable,
            app_id,
            active,
            windows: vec![window],
            ..Default::default()
        });
    }
}

fn unique_app(apps: &[DockApp], matches: impl Fn(&DockApp) -> bool) -> Option<usize> {
    let mut matching = apps.iter().enumerate().filter(|(_, app)| matches(app));
    let (index, _) = matching.next()?;
    matching.next().is_none().then_some(index)
}

fn has_distinct_app_id(id: Option<&str>) -> bool {
    id.is_some_and(|id| id.contains('!') || id.to_ascii_lowercase().contains("._crx_"))
}

pub fn activate_window(target: &DockWindow) -> bool {
    let window = HWND(target.handle as isize as *mut c_void);
    let mut process = 0;
    unsafe {
        if GetWindowThreadProcessId(window, Some(&mut process)) == 0 || process != target.process_id
        {
            return false;
        }
        if IsIconic(window).as_bool() {
            let _ = ShowWindowAsync(window, SW_RESTORE);
        }
        if SetForegroundWindow(window).as_bool() || GetForegroundWindow() == window {
            return true;
        }
        SwitchToThisWindow(window, true);
        GetForegroundWindow() == window
    }
}

fn window_app_id(window: HWND) -> Option<String> {
    let store: IPropertyStore = unsafe { SHGetPropertyStoreForWindow(window) }.ok()?;
    let value = unsafe { store.GetValue(&APP_USER_MODEL_ID) }.ok()?;
    read_string(unsafe { PropVariantToStringAlloc(&value) }.ok()?)
}

#[repr(C)]
struct ExplorerPinnedListVtable {
    base: IUnknown_Vtbl,
    enum_objects: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

fn shell_pins() -> Option<Vec<DockApp>> {
    let object: IUnknown =
        unsafe { CoCreateInstance(&TASKBAND_PIN, None, CLSCTX_INPROC_SERVER) }.ok()?;
    let mut list = std::ptr::null_mut();
    unsafe { object.query(&PINNED_LIST_IID, &mut list) }
        .ok()
        .ok()?;
    if list.is_null() {
        return None;
    }
    // The queried pointer owns a COM reference. Only the enumeration slot is used.
    let list = unsafe { IUnknown::from_raw(list) };
    let vtable = unsafe { &**list.as_raw().cast::<*const ExplorerPinnedListVtable>() };
    let mut raw = std::ptr::null_mut();
    unsafe { (vtable.enum_objects)(list.as_raw(), &mut raw) }
        .ok()
        .ok()?;
    if raw.is_null() {
        return None;
    }
    let entries = unsafe { IEnumFullIDList::from_raw(raw) };
    let mut apps = Vec::new();
    loop {
        let mut pidls = [std::ptr::null_mut()];
        let mut fetched = 0;
        let status = unsafe { entries.Next(&mut pidls, Some(&mut fetched)) };
        if status.is_err() {
            return None;
        }
        let [pidl] = pidls;
        if status != S_OK || fetched != 1 || pidl.is_null() {
            break;
        }
        let item: windows::core::Result<IShellItem2> = unsafe { SHCreateItemFromIDList(pidl) };
        unsafe { CoTaskMemFree(Some(pidl.cast())) };
        if let Some(app) = item.ok().and_then(pin_from_item) {
            apps.push(app);
        }
    }
    Some(apps)
}

fn shortcut_pins() -> Vec<DockApp> {
    let Some(roaming) = std::env::var_os("APPDATA") else {
        return Vec::new();
    };
    let folder =
        Path::new(&roaming).join(r"Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar");
    let mut paths = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let parsing_name = HSTRING::from(path.as_os_str());
            let item = unsafe { SHCreateItemFromParsingName(&parsing_name, None) }.ok()?;
            pin_from_item(item)
        })
        .collect()
}

fn pin_from_item(item: IShellItem2) -> Option<DockApp> {
    let name = read_string(unsafe { item.GetDisplayName(SIGDN_NORMALDISPLAY) }.ok()?)?;
    let mut app_id = unsafe { item.GetString(&APP_USER_MODEL_ID) }
        .ok()
        .and_then(read_string);
    let path = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }
        .ok()
        .and_then(read_string);
    let is_shortcut = path
        .as_deref()
        .is_some_and(|path| has_extension(path, "lnk"));
    let shortcut = path
        .as_deref()
        .filter(|_| is_shortcut)
        .and_then(read_shortcut);
    if app_id.is_none() {
        app_id = shortcut
            .as_ref()
            .and_then(|shortcut| shortcut.app_id.clone());
    }
    let executable = shortcut
        .as_ref()
        .and_then(|shortcut| shortcut.executable.clone())
        .or_else(|| {
            unsafe { item.GetString(&LINK_TARGET) }
                .ok()
                .and_then(read_string)
                .filter(|path| has_extension(path, "exe"))
        })
        .or_else(|| {
            path.as_ref()
                .filter(|path| has_extension(path, "exe"))
                .cloned()
        })
        .or_else(|| {
            if app_id
                .as_deref()
                .is_some_and(|id| equal_ignore_case(id, "Microsoft.Windows.Explorer"))
            {
                let windows = std::env::var_os("WINDIR")?;
                Some(
                    Path::new(&windows)
                        .join("explorer.exe")
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                None
            }
        });
    let target = path
        .or_else(|| app_id.as_ref().map(|id| format!(r"shell:AppsFolder\{id}")))
        .or_else(|| {
            unsafe { item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING) }
                .ok()
                .and_then(read_string)
        })?;
    let default_icon = if is_shortcut {
        executable.clone().unwrap_or_default()
    } else {
        target.clone()
    };
    let (icon_path, icon_index) = shortcut
        .as_ref()
        .and_then(|link| link.icon.clone())
        .map_or((default_icon, None), |(path, index)| (path, Some(index)));
    Some(DockApp {
        id: format!("pin:{target}"),
        name,
        icon_path,
        icon_index,
        launch_target: Some(target),
        app_id,
        executable: executable.unwrap_or_default(),
        distinct_shortcut: shortcut.is_some_and(|link| link.distinct),
        ..Default::default()
    })
}

#[derive(Default)]
struct Shortcut {
    executable: Option<String>,
    icon: Option<(String, i32)>,
    app_id: Option<String>,
    distinct: bool,
}

fn read_shortcut(path: &str) -> Option<Shortcut> {
    let link: IShellLinkW =
        unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.ok()?;
    let persist: IPersistFile = link.cast().ok()?;
    unsafe { persist.Load(&HSTRING::from(path), STGM_READ) }.ok()?;
    let mut text = [0u16; 520];
    let executable = unsafe { link.GetPath(&mut text, std::ptr::null_mut(), 0) }
        .ok()
        .and_then(|()| buffer_text(&text))
        .filter(|path| has_extension(path, "exe"));
    text.fill(0);
    let mut index = 0;
    let icon = unsafe { link.GetIconLocation(&mut text, &mut index) }
        .ok()
        .and_then(|()| buffer_text(&text))
        .filter(|path| !has_extension(path, "lnk") && Path::new(path).is_file())
        .map(|path| (path, index));
    text.fill(0);
    let distinct = unsafe { link.GetArguments(&mut text) }
        .ok()
        .and_then(|()| buffer_text(&text))
        .is_some_and(|args| {
            ["--app=", "--app-id="]
                .iter()
                .any(|flag| args.contains(flag))
        });
    let app_id = link
        .cast::<IPropertyStore>()
        .ok()
        .and_then(|store| unsafe { store.GetValue(&APP_USER_MODEL_ID) }.ok())
        .and_then(|value| unsafe { PropVariantToStringAlloc(&value) }.ok())
        .and_then(read_string);
    Some(Shortcut {
        executable,
        icon,
        app_id,
        distinct,
    })
}

fn has_extension(path: &str, extension: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

fn buffer_text(buffer: &[u16]) -> Option<String> {
    let end = buffer
        .iter()
        .position(|&value| value == 0)
        .unwrap_or(buffer.len());
    (end != 0).then(|| String::from_utf16_lossy(&buffer[..end]))
}

fn read_string(value: PWSTR) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let text = unsafe { value.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    text.filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{merge_running_window, DockApp, DockWindow};

    #[test]
    fn a_running_pin_keeps_its_position_and_does_not_duplicate() {
        let mut apps = vec![DockApp {
            id: "pin".into(),
            name: "Pinned App".into(),
            executable: r"C:\Tools\app.exe".into(),
            launch_target: Some("pin.lnk".into()),
            ..Default::default()
        }];
        merge_running_window(
            &mut apps,
            None,
            r"c:\tools\APP.EXE".into(),
            "Running App".into(),
            DockWindow {
                handle: 21,
                process_id: 3,
                title: "Document".into(),
            },
            true,
        );
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Pinned App");
        assert_eq!(apps[0].windows[0].handle, 21);
        assert!(apps[0].active);
    }

    #[test]
    fn a_unique_desktop_pin_matches_a_running_window_with_a_different_app_id() {
        let mut apps = vec![DockApp {
            id: "pin:Explorer".into(),
            name: "File Explorer".into(),
            executable: r"C:\Windows\explorer.exe".into(),
            app_id: Some("Microsoft.Windows.Explorer".into()),
            launch_target: Some("Explorer.lnk".into()),
            ..Default::default()
        }];
        merge_running_window(
            &mut apps,
            Some("Microsoft.Windows.Explorer.Process".into()),
            r"c:\windows\EXPLORER.EXE".into(),
            "File Explorer".into(),
            DockWindow {
                handle: 33,
                process_id: 4,
                title: "Downloads".into(),
            },
            true,
        );
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].id, "pin:Explorer");
        assert_eq!(apps[0].windows[0].handle, 33);
    }

    #[test]
    fn a_link_uses_its_apps_icon_and_keeps_the_shortcut_for_launch() {
        use super::*;

        let apartment = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        apartment.ok().unwrap();
        let exe = std::env::current_exe().unwrap();
        let shortcut =
            std::env::temp_dir().join(format!("OmniSpot-test-pin-{}.lnk", std::process::id()));
        let link: IShellLinkW =
            unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.unwrap();
        let persist: IPersistFile = link.cast().unwrap();
        let path = HSTRING::from(shortcut.as_os_str());
        unsafe {
            link.SetPath(&HSTRING::from(exe.as_os_str())).unwrap();
            persist.Save(&path, true).unwrap();
        }
        let item: IShellItem2 = unsafe { SHCreateItemFromParsingName(&path, None) }.unwrap();
        let app = pin_from_item(item).unwrap();
        drop(persist);
        drop(link);
        unsafe { CoUninitialize() };
        std::fs::remove_file(&shortcut).unwrap();
        assert_eq!(app.launch_target.as_deref(), shortcut.to_str());
        assert_eq!(app.icon_path, exe.to_str().unwrap());
        assert_eq!(app.executable, exe.to_str().unwrap());
    }
}
