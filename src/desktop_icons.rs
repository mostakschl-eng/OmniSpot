use std::mem::size_of;
use windows::core::PCWSTR;
use windows::Win32::{
    Foundation::HANDLE,
    Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GdiFlush, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HBRUSH, HDC,
    },
    Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
    UI::{
        Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON},
        WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL},
    },
};

pub const ICON_SIZE: u32 = 32;

pub fn shell_icon(path: &str) -> Option<Vec<u8>> {
    let path: Vec<u16> = path.encode_utf16().chain([0]).collect();
    let mut info = SHFILEINFOW::default();
    unsafe {
        if SHGetFileInfoW(
            PCWSTR(path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES::default(),
            Some(&mut info),
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON,
        ) == 0
            || info.hIcon.is_invalid()
        {
            return None;
        }
        let size = ICON_SIZE as i32;
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
                    let result = DrawIconEx(
                        dc,
                        0,
                        0,
                        info.hIcon,
                        size,
                        size,
                        0,
                        HBRUSH::default(),
                        DI_NORMAL,
                    );
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
        let _ = DestroyIcon(info.hIcon);
        pixels
    }
}
