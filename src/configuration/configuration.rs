use super::app_theme_setting::AppThemeSetting;
use super::application_type::ApplicationType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CycleMode {
    NextApp,
    Hide,
    NextWindow,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
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

impl Default for Configuration {
    fn default() -> Self {
        Self {
            modifier: crate::input::keyboard_input::RIGHT_CTRL,
            applications: Vec::new(),
            pulse_border_enabled: true,
            theme: AppThemeSetting::System,
            overlay_enabled: false,
            overlay_show_delay_ms: 1000,
            overlay_keep_open_while_modifier_held: true,
            peek_enabled: false,
            dynamic_mode_enabled: true,
            stats_enabled: false,
            spotlight_enabled: true,
            spotlight_modifier: crate::input::keyboard_input::LEFT_ALT,
            spotlight_custom_directories: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationConfiguration {
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
