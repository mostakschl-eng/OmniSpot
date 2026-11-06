use crate::configuration::{
    ApplicationConfiguration, ApplicationType, CycleMode as ConfigCycleMode, Key,
};

#[derive(Clone)]
pub struct ApplicationShortcut {
    pub key: String,
    pub display_name: String,
    pub process_path: String,
    pub cycle_mode: CycleMode,
    pub start_if_not_running: bool,
    pub aumid: Option<String>,
}

impl ApplicationShortcut {
    pub fn from_configuration(config: &ApplicationConfiguration) -> Self {
        Self {
            key: format!("{:?}", config.key),
            display_name: config.process_name().to_string(),
            process_path: config.process_path.clone(),
            cycle_mode: config.cycle_mode.into(),
            start_if_not_running: config.start_if_not_running,
            aumid: config.aumid.clone(),
        }
    }
    pub fn to_configuration(&self) -> Option<ApplicationConfiguration> {
        Some(ApplicationConfiguration {
            key: parse_key(&self.key)?,
            process_path: self.process_path.clone(),
            cycle_mode: self.cycle_mode.into(),
            start_if_not_running: self.start_if_not_running,
            application_type: if self.aumid.is_some() {
                ApplicationType::Packaged
            } else {
                ApplicationType::Win32
            },
            aumid: self.aumid.clone(),
        })
    }
}

fn parse_key(value: &str) -> Option<Key> {
    Some(match value {
        "A" => Key::A,
        "B" => Key::B,
        "C" => Key::C,
        "D" => Key::D,
        "E" => Key::E,
        "F" => Key::F,
        "G" => Key::G,
        "H" => Key::H,
        "I" => Key::I,
        "J" => Key::J,
        "K" => Key::K,
        "L" => Key::L,
        "M" => Key::M,
        "N" => Key::N,
        "O" => Key::O,
        "P" => Key::P,
        "Q" => Key::Q,
        "R" => Key::R,
        "S" => Key::S,
        "T" => Key::T,
        "U" => Key::U,
        "V" => Key::V,
        "W" => Key::W,
        "X" => Key::X,
        "Y" => Key::Y,
        "Z" => Key::Z,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CycleMode {
    NextApp,
    Hide,
    NextWindow,
}

impl From<ConfigCycleMode> for CycleMode {
    fn from(value: ConfigCycleMode) -> Self {
        match value {
            ConfigCycleMode::NextApp => Self::NextApp,
            ConfigCycleMode::Hide => Self::Hide,
            ConfigCycleMode::NextWindow => Self::NextWindow,
        }
    }
}
impl From<CycleMode> for ConfigCycleMode {
    fn from(value: CycleMode) -> Self {
        match value {
            CycleMode::NextApp => Self::NextApp,
            CycleMode::Hide => Self::Hide,
            CycleMode::NextWindow => Self::NextWindow,
        }
    }
}
