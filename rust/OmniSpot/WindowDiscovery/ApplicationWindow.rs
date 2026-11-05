use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationWindow {
    pub handle: isize,
    pub title: String,
    pub process_id: u32,
    pub process_image_path: String,
    pub is_minimized: bool,
    pub is_cloaked: bool,
    pub extended_style: u32,
    pub style: u32,
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub needs_elevation: bool,
}

impl ApplicationWindow {
    pub fn process_name(&self) -> &str {
        Path::new(&self.process_image_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&self.process_image_path)
    }
    pub fn is_valid(&self) -> bool {
        !self.title.is_empty()
            && self.size.0 > 0
            && self.size.1 > 0
            && !self.is_cloaked
            && self.extended_style & 0x08000000 == 0
            && self.extended_style & 0x00000080 == 0
    }
}
