use crate::configuration::model::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsDocument {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub modifier: Key,
    #[serde(default)]
    pub applications: Vec<ApplicationConfigurationDocument>,
    #[serde(default)]
    pub pulse_border_enabled: bool,
    #[serde(default)]
    pub theme: AppThemeSetting,
    #[serde(default)]
    pub overlay_enabled: bool,
    #[serde(default = "default_delay")]
    pub overlay_show_delay_ms: u32,
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
    pub spotlight_modifier: Key,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationConfigurationDocument {
    pub key: Key,
    pub process_path: String,
    pub cycle_mode: CycleMode,
    pub start_if_not_running: bool,
    #[serde(default)]
    pub application_type: ApplicationType,
    #[serde(default)]
    pub aumid: Option<String>,
}

fn default_schema_version() -> u32 {
    1
}
fn default_delay() -> u32 {
    1000
}
fn default_true() -> bool {
    true
}
fn default_spotlight_modifier() -> Key {
    Key::LeftAlt
}

impl SettingsDocument {
    pub fn from_configuration(config: &Configuration) -> Self {
        Self {
            schema_version: 2,
            modifier: config.modifier,
            applications: config.applications.iter().map(Into::into).collect(),
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
        }
    }

    pub fn to_configuration(&self) -> Configuration {
        Configuration {
            modifier: self.modifier,
            applications: self.applications.iter().map(Into::into).collect(),
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
        }
    }
}

impl From<&ApplicationConfiguration> for ApplicationConfigurationDocument {
    fn from(value: &ApplicationConfiguration) -> Self {
        Self {
            key: value.key,
            process_path: value.process_path.clone(),
            cycle_mode: value.cycle_mode,
            start_if_not_running: value.start_if_not_running,
            application_type: value.application_type,
            aumid: value.aumid.clone(),
        }
    }
}
impl From<&ApplicationConfigurationDocument> for ApplicationConfiguration {
    fn from(value: &ApplicationConfigurationDocument) -> Self {
        Self {
            key: value.key,
            process_path: value.process_path.clone(),
            cycle_mode: value.cycle_mode,
            start_if_not_running: value.start_if_not_running,
            application_type: value.application_type,
            aumid: value.aumid.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_all_configuration_fields() {
        let original = Configuration {
            modifier: Key::LeftAlt,
            applications: vec![ApplicationConfiguration {
                key: Key::N,
                process_path: r"C:\Windows\notepad.exe".to_string(),
                cycle_mode: CycleMode::NextWindow,
                start_if_not_running: true,
                application_type: ApplicationType::Packaged,
                aumid: Some("Microsoft.WindowsNotepad_8wekyb3d8bbwe!App".to_string()),
            }],
            pulse_border_enabled: false,
            theme: AppThemeSetting::Dark,
            overlay_enabled: true,
            overlay_show_delay_ms: 500,
            overlay_keep_open_while_modifier_held: false,
            peek_enabled: true,
            dynamic_mode_enabled: false,
            stats_enabled: true,
            spotlight_enabled: true,
            spotlight_modifier: Key::LeftCtrl,
        };

        let document = SettingsDocument::from_configuration(&original);

        assert_eq!(document.to_configuration(), original);
    }
}
