use std::{
    ffi::OsStr,
    fs,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use windows::Win32::{
    Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC,
        GetDIBits, GetObjectW, HGDIOBJ, ReleaseDC,
    },
    UI::{
        Shell::ExtractIconExW,
        WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO},
    },
};
use windows::core::PCWSTR;

/// Extracts associated shell icons once into the user's cache directory.
#[derive(Default)]
pub struct IconExtractor;

impl IconExtractor {
    pub fn get_by_process_path(&self, process_path: &str) -> Option<String> {
        self.get_by_path(Path::new(process_path))
    }

    pub fn get_by_path(&self, path: &Path) -> Option<String> {
        if !path.exists() {
            return None;
        }
        let cache = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("OmniSpot")
            .join("icons");
        let key = format!("{:016x}", stable_path_hash(path));
        let output = cache.join(format!("{key}.png"));
        if output.exists() {
            return Some(output.to_string_lossy().into_owned());
        }
        fs::create_dir_all(&cache).ok()?;
        let png = extract_icon_png(path)?;
        fs::write(&output, png).ok()?;
        Some(output.to_string_lossy().into_owned())
    }
}

fn stable_path_hash(path: &Path) -> u64 {
    let mut hash = 14695981039346656037u64;
    for byte in path.to_string_lossy().to_ascii_lowercase().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

fn extract_icon_png(path: &Path) -> Option<Vec<u8>> {
    let wide = OsStr::new(path)
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut icon = HICON::default();
    let count = unsafe { ExtractIconExW(PCWSTR(wide.as_ptr()), 0, Some(&mut icon), None, 1) };
    if count == 0 || icon.0.is_null() {
        return None;
    }
    let result = icon_to_png(icon);
    unsafe {
        let _ = DestroyIcon(icon);
    }
    result
}

fn icon_to_png(icon: HICON) -> Option<Vec<u8>> {
    let mut info = ICONINFO::default();
    unsafe { GetIconInfo(icon, &mut info).ok()? };
    let result = bitmap_to_png(info.hbmColor);
    unsafe {
        if !info.hbmColor.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(info.hbmColor.0));
        }
        if !info.hbmMask.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(info.hbmMask.0));
        }
    }
    result
}

fn bitmap_to_png(bitmap: windows::Win32::Graphics::Gdi::HBITMAP) -> Option<Vec<u8>> {
    if bitmap.0.is_null() {
        return None;
    }
    let mut dimensions = BITMAP::default();
    let copied = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut dimensions as *mut BITMAP).cast()),
        )
    };
    if copied == 0 || dimensions.bmWidth <= 0 || dimensions.bmHeight <= 0 {
        return None;
    }
    let width = dimensions.bmWidth as u32;
    let height = dimensions.bmHeight as u32;
    let mut bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: dimensions.bmWidth,
            biHeight: -dimensions.bmHeight,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0u8; width as usize * height as usize * 4];
    let hdc = unsafe { GetDC(None) };
    if hdc.0.is_null() {
        return None;
    }
    let scan_lines = unsafe {
        GetDIBits(
            hdc,
            bitmap,
            0,
            height,
            Some(bgra.as_mut_ptr().cast()),
            &mut bitmap_info,
            DIB_RGB_COLORS,
        )
    };
    unsafe {
        let _ = ReleaseDC(None, hdc);
    }
    if scan_lines == 0 {
        return None;
    }
    for pixel in bgra.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        if pixel[3] == 0 {
            pixel[3] = 255;
        }
    }
    let mut output = Vec::new();
    let mut encoder = png::Encoder::new(&mut output, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().ok()?.write_image_data(&bgra).ok()?;
    Some(output)
}
