use super::window_style::{WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApplicationWindow {
    pub handle: i64,
    pub title: Option<String>,
    pub process_id: u32,
    pub process_image_path: String,
    pub state: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub style: u32,
    pub style_ex: u32,
    pub is_cloaked: bool,
    pub needs_elevation: bool,
}

impl ApplicationWindow {
    pub fn is_valid_window(&self) -> bool {
        (self.width != 0 || self.height != 0)
            && self.title.as_ref().is_some_and(|title| !title.is_empty())
            && self.style_ex & WS_EX_NOACTIVATE == 0
            && self.style_ex & WS_EX_TOOLWINDOW == 0
            && !self.is_cloaked
    }
}
