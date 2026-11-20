use serde::{Deserialize, Serialize};
use super::super::application_type::ApplicationType;
use super::super::configuration::{ApplicationConfiguration, CycleMode};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ApplicationConfigurationDocument {
    pub key: u32,
    pub process_path: String,
    pub cycle_mode: CycleMode,
    pub start_if_not_running: bool,
    #[serde(default = "default_application_type")]
    pub r#type: ApplicationType,
    pub aumid: Option<String>,
}

fn default_application_type() -> ApplicationType {
    ApplicationType::Win32
}

impl ApplicationConfigurationDocument {
    pub fn to_application_configuration(&self) -> ApplicationConfiguration {
        ApplicationConfiguration {
            key: self.key,
            process_path: self.process_path.clone(),
            cycle_mode: self.cycle_mode,
            start_if_not_running: self.start_if_not_running,
            application_type: self.r#type,
            aumid: self.aumid.clone(),
        }
    }

    pub fn from_application_configuration(config: &ApplicationConfiguration) -> Self {
        Self {
            key: config.key,
            process_path: config.process_path.clone(),
            cycle_mode: config.cycle_mode,
            start_if_not_running: config.start_if_not_running,
            r#type: config.application_type,
            aumid: config.aumid.clone(),
        }
    }
}
