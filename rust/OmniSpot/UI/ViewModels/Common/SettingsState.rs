use super::application_shortcut::ApplicationShortcut;
use crate::configuration::{
    AppThemeSetting, Configuration, ConfigurationManager, ConfigurationService, Key, SettingsPath,
};
use crate::startup::auto_start::AutoStart;
use std::path::PathBuf;

pub(crate) struct SettingsState {
    pub applications: Vec<ApplicationShortcut>,
    pub theme: &'static str,
    pub modifier: &'static str,
    pub spotlight_modifier: &'static str,
    pub launch_at_startup: bool,
    pub dynamic_mode: bool,
    pub pulse_border: bool,
    pub peek_mode: bool,
    pub overlay_enabled: bool,
    pub overlay_delay_ms: u16,
    pub overlay_keep_open: bool,
    pub stats_enabled: bool,
    pub spotlight_enabled: bool,
    manager: ConfigurationManager,
}

impl Default for SettingsState {
    fn default() -> Self {
        let executable_dir = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."));
        let portable = executable_dir.join(".portable").exists();
        let path = SettingsPath::resolve(&executable_dir, portable)
            .unwrap_or_else(|_| PathBuf::from("settings.json"));
        let mut manager = ConfigurationManager::new(ConfigurationService::new(path));
        let config = manager.load().unwrap_or_default();
        Self::from_configuration(config, manager)
    }
}

impl SettingsState {
    pub fn set_launch_at_startup(&mut self, enabled: bool) -> Result<(), String> {
        let result = if enabled {
            AutoStart::create_shortcut()
        } else {
            AutoStart::remove_shortcut()
        };
        result.map_err(|error| format!("Could not update startup shortcut: {error}"))?;
        self.launch_at_startup = enabled;
        Ok(())
    }

    fn from_configuration(config: Configuration, manager: ConfigurationManager) -> Self {
        let mut state = Self {
            applications: Vec::new(),
            theme: "System (follow Windows)",
            modifier: "RightCtrl",
            spotlight_modifier: "LeftAlt",
            launch_at_startup: AutoStart::is_enabled().unwrap_or(false),
            dynamic_mode: true,
            pulse_border: true,
            peek_mode: false,
            overlay_enabled: false,
            overlay_delay_ms: 1000,
            overlay_keep_open: true,
            stats_enabled: false,
            spotlight_enabled: true,
            manager,
        };
        state.apply_configuration(config);
        state
    }

    fn apply_configuration(&mut self, config: Configuration) {
        self.applications = config
            .applications
            .iter()
            .map(ApplicationShortcut::from_configuration)
            .collect();
        self.theme = match config.theme {
            AppThemeSetting::System => "System (follow Windows)",
            AppThemeSetting::Dark => "Dark",
            AppThemeSetting::Light => "Light",
        };
        self.modifier = format!("{:?}", config.modifier).leak();
        self.spotlight_modifier = format!("{:?}", config.spotlight_modifier).leak();
        self.dynamic_mode = config.dynamic_mode_enabled;
        self.pulse_border = config.pulse_border_enabled;
        self.peek_mode = config.peek_enabled;
        self.overlay_enabled = config.overlay_enabled;
        self.overlay_delay_ms = config.overlay_show_delay_ms.min(u16::MAX as u32) as u16;
        self.overlay_keep_open = config.overlay_keep_open_while_modifier_held;
        self.stats_enabled = config.stats_enabled;
        self.spotlight_enabled = config.spotlight_enabled;
    }

    pub fn save(&mut self) -> Result<Configuration, String> {
        let configuration = self.to_configuration()?;
        self.manager
            .update(configuration.clone())
            .map_err(|error| format!("Could not save settings: {error:?}"))?;
        Ok(configuration)
    }

    pub fn reset(&mut self) -> Result<(), String> {
        let config = self
            .manager
            .load()
            .map_err(|error| format!("Could not reload settings: {error:?}"))?;
        self.apply_configuration(config);
        Ok(())
    }

    fn to_configuration(&self) -> Result<Configuration, String> {
        let applications = self
            .applications
            .iter()
            .map(ApplicationShortcut::to_configuration)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| "Application shortcut contains an invalid key".to_string())?;
        Ok(Configuration {
            modifier: parse_modifier(self.modifier),
            applications,
            pulse_border_enabled: self.pulse_border,
            theme: match self.theme {
                "Dark" => AppThemeSetting::Dark,
                "Light" => AppThemeSetting::Light,
                _ => AppThemeSetting::System,
            },
            overlay_enabled: self.overlay_enabled,
            overlay_show_delay_ms: self.overlay_delay_ms as u32,
            overlay_keep_open_while_modifier_held: self.overlay_keep_open,
            peek_enabled: self.peek_mode,
            dynamic_mode_enabled: self.dynamic_mode,
            stats_enabled: self.stats_enabled,
            spotlight_enabled: self.spotlight_enabled,
            spotlight_modifier: parse_modifier(self.spotlight_modifier),
        })
    }
}

fn parse_modifier(value: &str) -> Key {
    match value {
        "LeftCtrl" => Key::LeftCtrl,
        "LeftAlt" => Key::LeftAlt,
        "LeftShift" => Key::LeftShift,
        "LWin" => Key::LWin,
        "RightCtrl" => Key::RightCtrl,
        "RightAlt" => Key::RightAlt,
        "RightShift" => Key::RightShift,
        "Apps" => Key::Apps,
        "Capital" | "CapsLock" => Key::Capital,
        _ => Key::RightCtrl,
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsState;
    #[test]
    fn defaults_load_with_expected_profile_values() {
        let state = SettingsState::default();
        assert_eq!(state.modifier, "RightCtrl");
        assert_eq!(state.spotlight_modifier, "LeftAlt");
        assert_eq!(state.overlay_delay_ms, 1000);
        assert!(state.dynamic_mode);
        assert!(state.pulse_border);
        assert!(!state.stats_enabled);
    }
}
