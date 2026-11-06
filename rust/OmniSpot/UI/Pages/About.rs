use crate::{
    ui::{
        SettingsWindow, controls::settings_card::settings_card, view_models::about::AboutViewModel,
    },
    window_discovery::{WindowEnumerator, process_inspector::ProcessInspector},
};
use gpui::{Context, IntoElement, div, img, prelude::*, px, rgb};
use std::{collections::BTreeSet, ffi::OsStr, os::windows::ffi::OsStrExt, path::PathBuf};
use windows::{
    Win32::{
        Foundation::{GlobalFree, HANDLE},
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock},
        },
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    },
    core::{PCWSTR, w},
};

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn open_path(path: &str) -> Result<(), String> {
    let path = wide(path);
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as usize <= 32 {
        Err("Windows could not open the requested location.".into())
    } else {
        Ok(())
    }
}

fn log_folder() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("OmniSpot")
        .join("logs")
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn style_names(value: u32, extended: bool) -> String {
    let standard = [
        (0, "WS_OVERLAPPED"),
        (0, "WS_TILED"),
        (1, "WS_ACTIVECAPTION"),
        (0x8000_0000, "WS_POPUP"),
        (0x4000_0000, "WS_CHILD"),
        (0x4000_0000, "WS_CHILDWINDOW"),
        (0x2000_0000, "WS_MINIMIZE"),
        (0x2000_0000, "WS_ICONIC"),
        (0x1000_0000, "WS_VISIBLE"),
        (0x0800_0000, "WS_DISABLED"),
        (0x0400_0000, "WS_CLIPSIBLINGS"),
        (0x0200_0000, "WS_CLIPCHILDREN"),
        (0x0100_0000, "WS_MAXIMIZE"),
        (0x00C0_0000, "WS_CAPTION"),
        (0x0080_0000, "WS_BORDER"),
        (0x0040_0000, "WS_DLGFRAME"),
        (0x0020_0000, "WS_VSCROLL"),
        (0x0010_0000, "WS_HSCROLL"),
        (0x0008_0000, "WS_SYSMENU"),
        (0x0004_0000, "WS_THICKFRAME"),
        (0x0004_0000, "WS_SIZEBOX"),
        (0x0002_0000, "WS_GROUP"),
        (0x0002_0000, "WS_MINIMIZEBOX"),
        (0x0001_0000, "WS_TABSTOP"),
        (0x0001_0000, "WS_MAXIMIZEBOX"),
        (0x00CF_0000, "WS_OVERLAPPEDWINDOW"),
        (0x00CF_0000, "WS_TILEDWINDOW"),
        (0x8088_0000, "WS_POPUPWINDOW"),
    ];
    let extended_flags = [
        (0x10, "WS_EX_ACCEPTFILES"),
        (0x0004_0000, "WS_EX_APPWINDOW"),
        (0x200, "WS_EX_CLIENTEDGE"),
        (0x0200_0000, "WS_EX_COMPOSITED"),
        (0x400, "WS_EX_CONTEXTHELP"),
        (0x0001_0000, "WS_EX_CONTROLPARENT"),
        (1, "WS_EX_DLGMODALFRAME"),
        (0x0008_0000, "WS_EX_LAYERED"),
        (0x0040_0000, "WS_EX_LAYOUTRTL"),
        (0, "WS_EX_LEFT"),
        (0x4000, "WS_EX_LEFTSCROLLBAR"),
        (0, "WS_EX_LTRREADING"),
        (0x40, "WS_EX_MDICHILD"),
        (0x0800_0000, "WS_EX_NOACTIVATE"),
        (0x0010_0000, "WS_EX_NOINHERITLAYOUT"),
        (4, "WS_EX_NOPARENTNOTIFY"),
        (0x0020_0000, "WS_EX_NOREDIRECTIONBITMAP"),
        (0x300, "WS_EX_OVERLAPPEDWINDOW"),
        (0x188, "WS_EX_PALETTEWINDOW"),
        (0x1000, "WS_EX_RIGHT"),
        (0, "WS_EX_RIGHTSCROLLBAR"),
        (0x2000, "WS_EX_RTLREADING"),
        (0x0002_0000, "WS_EX_STATICEDGE"),
        (0x80, "WS_EX_TOOLWINDOW"),
        (8, "WS_EX_TOPMOST"),
        (0x20, "WS_EX_TRANSPARENT"),
        (0x100, "WS_EX_WINDOWEDGE"),
    ];
    let flags = if extended {
        &extended_flags[..]
    } else {
        &standard[..]
    };
    flags
        .iter()
        .filter(|(mask, _)| *mask == 0 || value & mask == *mask)
        .map(|(_, name)| *name)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(" | ")
}

fn diagnostics_csv() -> Result<(String, usize), String> {
    let windows = WindowEnumerator::default().get_all_windows();
    let mut csv = String::from(
        "ProcessId,Handle,ProcessImagePath,ProductName,Title,State,Style,StyleEx,IsCloaked\r\n",
    );
    for window in &windows {
        let product =
            ProcessInspector::product_name(&window.process_image_path).unwrap_or_default();
        let state = match window.show_state {
            0 => "SW_HIDE".to_string(),
            1 => "SW_SHOWNORMAL".to_string(),
            2 => "SW_SHOWMINIMIZED".to_string(),
            3 => "SW_SHOWMAXIMIZED".to_string(),
            4 => "SW_SHOWNOACTIVATE".to_string(),
            5 => "SW_SHOW".to_string(),
            6 => "SW_MINIMIZE".to_string(),
            7 => "SW_SHOWMINNOACTIVE".to_string(),
            8 => "SW_SHOWNA".to_string(),
            9 => "SW_RESTORE".to_string(),
            10 => "SW_SHOWDEFAULT".to_string(),
            11 => "SW_FORCEMINIMIZE".to_string(),
            value => value.to_string(),
        };
        let row = [
            window.process_id.to_string(),
            format!("0x{:X}", window.handle),
            window.process_image_path.clone(),
            product,
            window.title.clone(),
            state,
            style_names(window.style, false),
            style_names(window.extended_style, true),
            window.is_cloaked.to_string(),
        ];
        csv.push_str(
            &row.iter()
                .map(|value| csv_escape(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push_str("\r\n");
    }
    Ok((csv, windows.len()))
}

fn copy_to_clipboard(value: &str) -> Result<(), String> {
    let mut text = value.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, text.len() * std::mem::size_of::<u16>())
            .map_err(|error| error.to_string())?;
        let destination = GlobalLock(memory);
        if destination.is_null() {
            let _ = GlobalFree(Some(memory));
            return Err("Could not allocate clipboard memory.".into());
        }
        std::ptr::copy_nonoverlapping(text.as_mut_ptr(), destination.cast(), text.len());
        let _ = windows::Win32::System::Memory::GlobalUnlock(memory);
        OpenClipboard(None).map_err(|error| {
            let _ = GlobalFree(Some(memory));
            error.to_string()
        })?;
        let result = (|| {
            EmptyClipboard().map_err(|error| error.to_string())?;
            SetClipboardData(13, Some(HANDLE(memory.0))).map_err(|error| error.to_string())?;
            Ok(())
        })();
        let _ = CloseClipboard();
        if result.is_err() {
            let _ = GlobalFree(Some(memory));
        }
        result
    }
}

pub(crate) fn render(cx: &mut Context<SettingsWindow>) -> impl IntoElement {
    div()
        .id("about-scroll")
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .w_full()
                .min_h(px(100.0))
                .p_4()
                .rounded_md()
                .bg(rgb(0x303532))
                .flex()
                .items_center()
                .gap_5()
                .child(
                    AboutViewModel::app_icon()
                        .map(|path| {
                            img(PathBuf::from(path))
                                .w(px(52.0))
                                .h(px(52.0))
                                .into_any_element()
                        })
                        .unwrap_or_else(|| {
                            div()
                                .w(px(52.0))
                                .h(px(52.0))
                                .rounded_md()
                                .bg(rgb(0x414744))
                                .into_any_element()
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_xl().child(AboutViewModel::APP_NAME))
                        .child(AboutViewModel::app_version())
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(0xc1c4c2))
                                .child(AboutViewModel::RUNTIME),
                        ),
                ),
        )
        .child("Support")
        .child(settings_card(
            "Log folder",
            "Open the folder containing application logs",
            div()
                .id("open-log-folder")
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x414744))
                .cursor_pointer()
                .child("Open")
                .on_click(cx.listener(|this, _, _, cx| {
                    match open_path(&log_folder().to_string_lossy()) {
                        Ok(()) => this.save_status = Some(("Opened the log folder.".into(), true)),
                        Err(error) => {
                            this.save_status =
                                Some((format!("Could not open log folder: {error}"), false))
                        }
                    }
                    cx.notify();
                })),
        ))
        .child(settings_card(
            "Website",
            AboutViewModel::APP_WEBSITE,
            div()
                .id("open-website")
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x414744))
                .cursor_pointer()
                .child("Open")
                .on_click(cx.listener(|this, _, _, cx| {
                    match open_path(AboutViewModel::APP_WEBSITE) {
                        Ok(()) => {
                            this.save_status = Some(("Opened the OmniSpot website.".into(), true))
                        }
                        Err(error) => {
                            this.save_status =
                                Some((format!("Could not open website: {error}"), false))
                        }
                    }
                    cx.notify();
                })),
        ))
        .child(settings_card(
            "Window diagnostics",
            "Copy information about all open windows to the clipboard",
            div()
                .id("copy-window-diagnostics")
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(0x414744))
                .cursor_pointer()
                .child("Copy")
                .on_click(cx.listener(|this, _, _, cx| {
                    match diagnostics_csv()
                        .and_then(|(csv, count)| copy_to_clipboard(&csv).map(|()| count))
                    {
                        Ok(count) => {
                            this.save_status =
                                Some((format!("Copied diagnostics for {count} windows."), true))
                        }
                        Err(error) => {
                            this.save_status =
                                Some((format!("Could not copy window diagnostics: {error}"), false))
                        }
                    }
                    cx.notify();
                })),
        ))
}

#[cfg(test)]
mod tests {
    use super::{csv_escape, style_names};

    #[test]
    fn diagnostics_escape_csv_and_decode_window_style_flags() {
        assert_eq!(csv_escape("a,\"b\""), "\"a,\"\"b\"\"\"");
        assert!(style_names(0x1000_0000, false).contains("WS_VISIBLE"));
        assert!(style_names(0x0800_0000, true).contains("WS_EX_NOACTIVATE"));
    }
}
