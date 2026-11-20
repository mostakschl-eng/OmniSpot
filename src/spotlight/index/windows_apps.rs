use super::scan::is_uninstaller_or_helper;
use super::{create_entry, IndexedEntry};
use std::path::PathBuf;

pub(super) fn enumerate_shell_apps() -> Vec<IndexedEntry> {
    use windows::core::w;
    use windows::Win32::System::Com::{
        CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        BHID_EnumItems, IEnumShellItems, IShellItem, SHCreateItemFromParsingName,
        SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY,
    };

    let mut apps = Vec::new();

    unsafe {
        if CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_err() {
            return apps;
        }

        if let Ok(shell_folder) =
            SHCreateItemFromParsingName::<_, _, IShellItem>(w!("shell:AppsFolder"), None)
        {
            if let Ok(enum_items) =
                shell_folder.BindToHandler::<_, IEnumShellItems>(None, &BHID_EnumItems)
            {
                let mut item_buf = [None];
                let mut fetched = 0;
                while enum_items.Next(&mut item_buf, Some(&mut fetched)).is_ok() && fetched == 1 {
                    if let Some(item) = item_buf[0].take() {
                        let name = if let Ok(p) = item.GetDisplayName(SIGDN_NORMALDISPLAY) {
                            let s = p.to_string().unwrap_or_default();
                            CoTaskMemFree(Some(p.as_ptr() as _));
                            s
                        } else {
                            String::new()
                        };

                        let target =
                            if let Ok(p) = item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING) {
                                let s = p.to_string().unwrap_or_default();
                                CoTaskMemFree(Some(p.as_ptr() as _));
                                s
                            } else {
                                String::new()
                            };

                        if !name.is_empty()
                            && !target.is_empty()
                            && !name.starts_with('{')
                            && !target.ends_with(".url")
                        {
                            if is_uninstaller_or_helper(&name) || is_uninstaller_or_helper(&target)
                            {
                                continue;
                            }
                            let alt = name.replace(' ', "");
                            let base = if target.contains(".com-")
                                || target.contains(".org-")
                                || target.contains(".app-")
                            {
                                50
                            } else {
                                80
                            };
                            apps.push(create_entry(&name, &target, &alt, "App", base));
                        }
                    }
                }
            }
        }

        CoUninitialize();
    }

    apps
}

pub(super) fn seed_windows_tools() -> Vec<IndexedEntry> {
    let system = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    let system32 = PathBuf::from(&system).join("System32");

    let tools = [
        ("Control Panel", system32.join("control.exe"), "control"),
        ("Task Manager", system32.join("Taskmgr.exe"), "taskmgr"),
        (
            "File Explorer",
            PathBuf::from(&system).join("explorer.exe"),
            "explorer",
        ),
        ("Command Prompt", system32.join("cmd.exe"), "cmd"),
        (
            "PowerShell",
            system32.join("WindowsPowerShell\\v1.0\\powershell.exe"),
            "powershell",
        ),
        (
            "Registry Editor",
            PathBuf::from(&system).join("regedit.exe"),
            "regedit",
        ),
        ("Notepad", system32.join("notepad.exe"), "notepad"),
        ("Calculator", system32.join("calc.exe"), "calc"),
        (
            "Snipping Tool",
            system32.join("SnippingTool.exe"),
            "snippingtool",
        ),
        ("Device Manager", system32.join("devmgmt.msc"), "devmgmt"),
        ("Disk Management", system32.join("diskmgmt.msc"), "diskmgmt"),
        ("Services", system32.join("services.msc"), "services"),
        ("Event Viewer", system32.join("eventvwr.msc"), "eventvwr"),
        ("Resource Monitor", system32.join("resmon.exe"), "resmon"),
    ];

    let mut entries = Vec::with_capacity(tools.len() + 10);
    for (title, target, alt) in tools {
        entries.push(create_entry(
            title,
            target.to_string_lossy().as_ref(),
            alt,
            "App",
            90,
        ));
    }

    let settings = [
        ("Settings", "ms-settings:", "settings"),
        ("Windows Update", "ms-settings:windowsupdate", "update"),
        ("Apps & Features", "ms-settings:appsfeatures", "uninstall"),
        (
            "Display Settings",
            "ms-settings:display",
            "display screen resolution",
        ),
        ("Sound Settings", "ms-settings:sound", "sound audio volume"),
        (
            "Bluetooth & Devices",
            "ms-settings:bluetooth",
            "bluetooth devices",
        ),
        (
            "Network & Internet",
            "ms-settings:network",
            "wifi ethernet network internet",
        ),
        (
            "Personalization",
            "ms-settings:personalization",
            "themes wallpaper background",
        ),
        (
            "Date & Time",
            "ms-settings:dateandtime",
            "clock date time timezone",
        ),
    ];

    for (title, target, alt) in settings {
        entries.push(create_entry(title, target, alt, "App", 85));
    }

    entries
}
