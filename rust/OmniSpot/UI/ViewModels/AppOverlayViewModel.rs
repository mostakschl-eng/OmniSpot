#[derive(Default)]
pub struct OverlayState {
    pub window_groups: Vec<OverlayWindowGroup>,
    pub running: Vec<OverlayRow>,
    pub launchable: Vec<OverlayRow>,
    pub dynamic: Vec<OverlayRow>,
}

pub struct OverlayWindowGroup {
    pub app_name: String,
    pub windows: Vec<OverlayRow>,
}

pub struct OverlayRow {
    pub name: String,
    pub key: String,
    pub active: bool,
    pub elevated: bool,
}
