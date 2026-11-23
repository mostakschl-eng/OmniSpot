use std::{mem::size_of, path::Path};
use windows::core::PCWSTR;
use windows::Win32::{
    Foundation::{HANDLE, RPC_E_CHANGED_MODE},
    Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GdiFlush, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HBRUSH, HDC,
    },
    Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
    System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED},
    UI::{
        Controls::{IImageList, ILD_TRANSPARENT},
        Shell::{
            ExtractIconExW, SHDefExtractIconW, SHGetFileInfoW, SHGetImageList, SHParseDisplayName,
            SHFILEINFOW, SHGFI_FLAGS, SHGFI_ICON, SHGFI_PIDL, SHGFI_SYSICONINDEX, SHIL_EXTRALARGE,
        },
        WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL, HICON},
    },
};

pub const ICON_SIZE: u32 = 32;
pub const DOCK_ICON_SIZE: u32 = 64;

pub fn shell_icon(path: &str) -> Option<Vec<u8>> {
    let icon = shell_file_info(path, SHGFI_ICON)?.hIcon;
    if icon.is_invalid() {
        return None;
    }
    unsafe { render_icon(icon, ICON_SIZE) }
}

pub fn dock_icon(path: &str, index: Option<i32>) -> Option<Vec<u8>> {
    if path.is_empty()
        || Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
    {
        return None;
    }
    if Path::new(path).is_file() {
        let path: Vec<u16> = path.encode_utf16().chain([0]).collect();
        let mut icon = HICON::default();
        unsafe {
            let _ = SHDefExtractIconW(
                PCWSTR(path.as_ptr()),
                index.unwrap_or(0),
                0,
                Some(&mut icon),
                None,
                DOCK_ICON_SIZE,
            );
            if !icon.is_invalid() {
                return render_icon(icon, DOCK_ICON_SIZE);
            }
        }
    }

    let apartment = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let icon = (apartment.is_ok() || apartment == RPC_E_CHANGED_MODE)
        .then(|| shell_dock_icon(path))
        .flatten();
    if apartment.is_ok() {
        unsafe { CoUninitialize() };
    }
    icon
}

fn shell_dock_icon(path: &str) -> Option<Vec<u8>> {
    // Use the base image-list entry. Overlay bits are never requested.
    let info = shell_file_info(path, SHGFI_SYSICONINDEX)?;
    let list: IImageList = unsafe { SHGetImageList(SHIL_EXTRALARGE as i32) }.ok()?;
    let icon = unsafe { list.GetIcon(info.iIcon, ILD_TRANSPARENT.0) }.ok()?;
    if icon.is_invalid() {
        return None;
    }
    unsafe { render_icon(icon, DOCK_ICON_SIZE) }
}

fn shell_file_info(path: &str, flags: SHGFI_FLAGS) -> Option<SHFILEINFOW> {
    let shell_item = path.starts_with("shell:") || path.starts_with("::{");
    let path: Vec<u16> = path.encode_utf16().chain([0]).collect();
    let mut info = SHFILEINFOW::default();
    unsafe {
        let mut pidl = std::ptr::null_mut();
        let (item, flags) = if shell_item {
            SHParseDisplayName(PCWSTR(path.as_ptr()), None, &mut pidl, 0, None).ok()?;
            (PCWSTR(pidl.cast()), flags | SHGFI_PIDL)
        } else {
            (PCWSTR(path.as_ptr()), flags)
        };
        let loaded = SHGetFileInfoW(
            item,
            FILE_FLAGS_AND_ATTRIBUTES::default(),
            Some(&mut info),
            size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        if !pidl.is_null() {
            CoTaskMemFree(Some(pidl.cast()));
        }
        (loaded != 0).then_some(info)
    }
}

pub fn indexed_icon(path: &str, index: i32) -> Option<Vec<u8>> {
    let path: Vec<u16> = path.encode_utf16().chain([0]).collect();
    let mut icon = HICON::default();
    let extracted =
        unsafe { ExtractIconExW(PCWSTR(path.as_ptr()), index, Some(&mut icon), None, 1) };
    if extracted != 1 || icon.is_invalid() {
        return None;
    }
    unsafe { render_icon(icon, ICON_SIZE) }
}

unsafe fn render_icon(icon: HICON, size: u32) -> Option<Vec<u8>> {
    let size = size as i32;
    let desc = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        },
        ..Default::default()
    };
    let dc = CreateCompatibleDC(HDC::default());
    let mut bits = std::ptr::null_mut();
    let bitmap = CreateDIBSection(dc, &desc, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0);
    let pixels = bitmap.ok().and_then(|bitmap| {
        let prior = SelectObject(dc, bitmap);
        let result = if !prior.is_invalid() && !bits.is_null() {
            let surface =
                std::slice::from_raw_parts_mut(bits.cast::<u8>(), (size * size * 4) as usize);
            let mut render = |background| {
                surface.fill(background);
                let result =
                    DrawIconEx(dc, 0, 0, icon, size, size, 0, HBRUSH::default(), DI_NORMAL);
                let _ = GdiFlush();
                result.ok().map(|_| surface.to_vec())
            };
            render(0).zip(render(255)).map(|(mut black, white)| {
                for (dark, light) in black.chunks_exact_mut(4).zip(white.chunks_exact(4)) {
                    let alpha = 255 - light[1].saturating_sub(dark[1]);
                    let straight = |color| {
                        if alpha == 0 {
                            0
                        } else {
                            ((u32::from(color) * 255 + u32::from(alpha) / 2) / u32::from(alpha))
                                .min(255) as u8
                        }
                    };
                    dark.copy_from_slice(&[
                        straight(dark[2]),
                        straight(dark[1]),
                        straight(dark[0]),
                        alpha,
                    ]);
                }
                black
            })
        } else {
            None
        };
        if !prior.is_invalid() {
            SelectObject(dc, prior);
        }
        let _ = DeleteObject(bitmap);
        result
    });
    let _ = DeleteDC(dc);
    let _ = DestroyIcon(icon);
    pixels
}

#[cfg(test)]
mod tests {
    use super::{dock_icon, DOCK_ICON_SIZE};

    #[test]
    fn dock_icon_uses_a_larger_app_resource_and_rejects_shortcuts() {
        let windows = std::env::var_os("WINDIR").unwrap();
        let explorer = std::path::Path::new(&windows).join("explorer.exe");
        let rgba = dock_icon(explorer.to_str().unwrap(), None).expect("Explorer app icon");
        assert_eq!(rgba.len(), (DOCK_ICON_SIZE * DOCK_ICON_SIZE * 4) as usize);
        assert!(rgba.chunks_exact(4).any(|pixel| pixel[3] != 0));
        assert!(dock_icon("shortcut.LNK", Some(0)).is_none());
    }
}
