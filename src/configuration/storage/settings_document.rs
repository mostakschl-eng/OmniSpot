use serde::{Deserialize, Serialize};
use super::super::app_theme_setting::AppThemeSetting;
use super::super::configuration::{default_taskbar_modifier, Configuration};
use super::application_configuration_document::ApplicationConfigurationDocument;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SettingsDocument {
    #[serde(rename = "_id", default)]
    pub id: i32,
    pub modifier: u32,
    #[serde(default)]
    pub applications: Vec<ApplicationConfigurationDocument>,
    #[serde(default)]
    pub pulse_border_enabled: bool,
    #[serde(default)]
    pub theme: AppThemeSetting,
    #[serde(default)]
    pub overlay_enabled: bool,
    #[serde(default = "default_overlay_show_delay")]
    pub overlay_show_delay_ms: i32,
    #[serde(default)]
    pub overlay_keep_open_while_modifier_held: bool,
    #[serde(default)]
    pub peek_enabled: bool,
    #[serde(default = "default_true")]
    pub dynamic_mode_enabled: bool,
    #[serde(default)]
    pub stats_enabled: bool,
    #[serde(default = "default_true")]
    pub spotlight_enabled: bool,
    #[serde(default = "default_spotlight_modifier")]
    pub spotlight_modifier: u32,
    #[serde(default)]
    pub spotlight_custom_directories: Option<Vec<String>>,
    #[serde(default)]
    pub hide_windows_taskbar: bool,
    #[serde(default = "default_taskbar_modifier")]
    pub taskbar_modifier: u32,
}

fn default_true() -> bool {
    true
}

fn default_overlay_show_delay() -> i32 {
    1000
}

fn default_spotlight_modifier() -> u32 {
    crate::input::keyboard_input::LEFT_ALT
}

impl SettingsDocument {
    pub fn to_configuration(&self) -> Configuration {
        Configuration {
            modifier: self.modifier,
            applications: self
                .applications
                .iter()
                .map(|a| a.to_application_configuration())
                .collect(),
            pulse_border_enabled: self.pulse_border_enabled,
            theme: self.theme,
            overlay_enabled: self.overlay_enabled,
            overlay_show_delay_ms: self.overlay_show_delay_ms,
            overlay_keep_open_while_modifier_held: self.overlay_keep_open_while_modifier_held,
            peek_enabled: self.peek_enabled,
            dynamic_mode_enabled: self.dynamic_mode_enabled,
            stats_enabled: self.stats_enabled,
            spotlight_enabled: self.spotlight_enabled,
            spotlight_modifier: self.spotlight_modifier,
            spotlight_custom_directories: self.spotlight_custom_directories.clone(),
            hide_windows_taskbar: self.hide_windows_taskbar,
            taskbar_modifier: self.taskbar_modifier,
        }
    }

    pub fn from_configuration(config: &Configuration, id: i32) -> Self {
        Self {
            id,
            modifier: config.modifier,
            applications: config
                .applications
                .iter()
                .map(|a| ApplicationConfigurationDocument::from_application_configuration(a))
                .collect(),
            pulse_border_enabled: config.pulse_border_enabled,
            theme: config.theme,
            overlay_enabled: config.overlay_enabled,
            overlay_show_delay_ms: config.overlay_show_delay_ms,
            overlay_keep_open_while_modifier_held: config.overlay_keep_open_while_modifier_held,
            peek_enabled: config.peek_enabled,
            dynamic_mode_enabled: config.dynamic_mode_enabled,
            stats_enabled: config.stats_enabled,
            spotlight_enabled: config.spotlight_enabled,
            spotlight_modifier: config.spotlight_modifier,
            spotlight_custom_directories: match &config.spotlight_custom_directories {
                Some(dirs) if !dirs.is_empty() => Some(dirs.clone()),
                _ => None,
            },
            hide_windows_taskbar: config.hide_windows_taskbar,
            taskbar_modifier: config.taskbar_modifier,
        }
    }
}
