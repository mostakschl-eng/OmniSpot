use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Key {
    #[default]
    None,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
    D9,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Return,
    Escape,
    Space,
    LeftCtrl,
    LeftAlt,
    LeftShift,
    LWin,
    RightCtrl,
    RightAlt,
    RightShift,
    RWin,
    Apps,
    Capital,
}

impl Key {
    pub fn is_letter(self) -> bool {
        matches!(
            self,
            Self::A
                | Self::B
                | Self::C
                | Self::D
                | Self::E
                | Self::F
                | Self::G
                | Self::H
                | Self::I
                | Self::J
                | Self::K
                | Self::L
                | Self::M
                | Self::N
                | Self::O
                | Self::P
                | Self::Q
                | Self::R
                | Self::S
                | Self::T
                | Self::U
                | Self::V
                | Self::W
                | Self::X
                | Self::Y
                | Self::Z
        )
    }
    pub fn is_digit(self) -> bool {
        matches!(
            self,
            Self::D0
                | Self::D1
                | Self::D2
                | Self::D3
                | Self::D4
                | Self::D5
                | Self::D6
                | Self::D7
                | Self::D8
                | Self::D9
        )
    }
    pub fn letter_index(self) -> Option<u16> {
        match self {
            Self::A => Some(0),
            Self::B => Some(1),
            Self::C => Some(2),
            Self::D => Some(3),
            Self::E => Some(4),
            Self::F => Some(5),
            Self::G => Some(6),
            Self::H => Some(7),
            Self::I => Some(8),
            Self::J => Some(9),
            Self::K => Some(10),
            Self::L => Some(11),
            Self::M => Some(12),
            Self::N => Some(13),
            Self::O => Some(14),
            Self::P => Some(15),
            Self::Q => Some(16),
            Self::R => Some(17),
            Self::S => Some(18),
            Self::T => Some(19),
            Self::U => Some(20),
            Self::V => Some(21),
            Self::W => Some(22),
            Self::X => Some(23),
            Self::Y => Some(24),
            Self::Z => Some(25),
            _ => None,
        }
    }
    pub fn is_modifier(self) -> bool {
        matches!(
            self,
            Self::LeftCtrl
                | Self::LeftAlt
                | Self::LeftShift
                | Self::LWin
                | Self::RightCtrl
                | Self::RightAlt
                | Self::RightShift
                | Self::Apps
                | Self::Capital
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CycleMode {
    #[default]
    NextApp,
    Hide,
    NextWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ApplicationType {
    #[default]
    Win32,
    Packaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppThemeSetting {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationConfiguration {
    pub key: Key,
    pub process_path: String,
    pub cycle_mode: CycleMode,
    pub start_if_not_running: bool,
    #[serde(default)]
    pub application_type: ApplicationType,
    #[serde(default)]
    pub aumid: Option<String>,
}

impl ApplicationConfiguration {
    pub fn process_name(&self) -> &str {
        self.process_path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&self.process_path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configuration {
    pub modifier: Key,
    pub applications: Vec<ApplicationConfiguration>,
    pub pulse_border_enabled: bool,
    pub theme: AppThemeSetting,
    pub overlay_enabled: bool,
    pub overlay_show_delay_ms: u32,
    pub overlay_keep_open_while_modifier_held: bool,
    pub peek_enabled: bool,
    pub dynamic_mode_enabled: bool,
    pub stats_enabled: bool,
    #[serde(default = "default_true")]
    pub spotlight_enabled: bool,
    #[serde(default = "default_spotlight_modifier")]
    pub spotlight_modifier: Key,
}

fn default_true() -> bool {
    true
}
fn default_spotlight_modifier() -> Key {
    Key::LeftAlt
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            modifier: Key::RightCtrl,
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
            spotlight_modifier: Key::LeftAlt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_existing_application_defaults() {
        let config = Configuration::default();

        assert_eq!(config.modifier, Key::RightCtrl);
        assert!(config.pulse_border_enabled);
        assert_eq!(config.overlay_show_delay_ms, 1000);
        assert!(config.dynamic_mode_enabled);
        assert!(config.spotlight_enabled);
        assert_eq!(config.spotlight_modifier, Key::LeftAlt);
    }

    #[test]
    fn process_name_uses_windows_file_name() {
        let app = ApplicationConfiguration {
            key: Key::N,
            process_path: r"C:\Windows\notepad.exe".to_string(),
            cycle_mode: CycleMode::NextApp,
            start_if_not_running: false,
            application_type: ApplicationType::Win32,
            aumid: None,
        };

        assert_eq!(app.process_name(), "notepad.exe");
    }
}
