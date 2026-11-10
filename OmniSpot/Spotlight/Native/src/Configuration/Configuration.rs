use super::app_theme_setting::AppThemeSetting;
use super::application_type::ApplicationType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CycleMode {
    NextApp,
    Hide,
    NextWindow,
}

#[derive(Clone, Debug)]
pub(crate) struct Configuration {
    // WPF Key values are data here; keyboard conversion is a separate source-file port.
    pub modifier: u32,
    pub applications: Vec<ApplicationConfiguration>,
    pub pulse_border_enabled: bool,
    pub theme: AppThemeSetting,
    pub overlay_enabled: bool,
    pub overlay_show_delay_ms: i32,
    pub overlay_keep_open_while_modifier_held: bool,
    pub peek_enabled: bool,
    pub dynamic_mode_enabled: bool,
    pub stats_enabled: bool,
    pub spotlight_enabled: bool,
    pub spotlight_modifier: u32,
    pub spotlight_custom_directories: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub(crate) struct ApplicationConfiguration {
    pub key: u32,
    pub process_path: String,
    pub cycle_mode: CycleMode,
    pub start_if_not_running: bool,
    pub application_type: ApplicationType,
    pub aumid: Option<String>,
}

impl ApplicationConfiguration {
    pub fn new(
        key: u32,
        process_path: String,
        cycle_mode: CycleMode,
        start_if_not_running: bool,
    ) -> Self {
        Self {
            key,
            process_path,
            cycle_mode,
            start_if_not_running,
            application_type: ApplicationType::Win32,
            aumid: None,
        }
    }

    pub fn process_name(&self) -> &str {
        crate::runtime::file_name(&self.process_path)
    }
}

pub(crate) trait IApplicationConfiguration {
    fn process_name(&self) -> &str;
    fn key(&self) -> u32;
    fn process_path(&self) -> &str;
    fn cycle_mode(&self) -> CycleMode;
    fn start_if_not_running(&self) -> bool;
    fn application_type(&self) -> ApplicationType;
    fn aumid(&self) -> Option<&str>;
}

impl IApplicationConfiguration for ApplicationConfiguration {
    fn process_name(&self) -> &str {
        self.process_name()
    }
    fn key(&self) -> u32 {
        self.key
    }
    fn process_path(&self) -> &str {
        &self.process_path
    }
    fn cycle_mode(&self) -> CycleMode {
        self.cycle_mode
    }
    fn start_if_not_running(&self) -> bool {
        self.start_if_not_running
    }
    fn application_type(&self) -> ApplicationType {
        self.application_type
    }
    fn aumid(&self) -> Option<&str> {
        self.aumid.as_deref()
    }
}
