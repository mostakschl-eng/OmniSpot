pub struct ApplicationShortcut {
    pub key: String,
    pub display_name: String,
    pub process_path: String,
    pub cycle_mode: CycleMode,
    pub start_if_not_running: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CycleMode {
    NextApp,
    Hide,
    NextWindow,
}
