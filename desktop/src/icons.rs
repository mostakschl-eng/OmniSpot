use omnispot_spotlight_native::desktop::{shell_icon, ICON_SIZE};
use slint::{Image, SharedPixelBuffer};
use std::{cell::RefCell, collections::HashMap, path::Path};

thread_local! {
    static ICONS: RefCell<HashMap<String, Image>> = RefCell::default();
}

pub fn load(path: &str) -> Image {
    if path.is_empty() {
        return Image::default();
    }
    ICONS.with(|icons| {
        let mut icons = icons.borrow_mut();
        if let Some(image) = icons.get(path) {
            return image.clone();
        }
        let image = match Path::new(path).extension().and_then(|part| part.to_str()) {
            Some(extension) if extension.eq_ignore_ascii_case("png") => {
                Image::load_from_path(Path::new(path)).ok()
            }
            _ => None,
        }
        .or_else(|| {
            shell_icon(path).map(|pixels| {
                Image::from_rgba8(SharedPixelBuffer::clone_from_slice(&pixels, ICON_SIZE, ICON_SIZE))
            })
        })
        .unwrap_or_default();
        if icons.len() >= 512 {
            icons.clear();
        }
        icons.insert(path.to_owned(), image.clone());
        image
    })
}
