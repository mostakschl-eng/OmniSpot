use super::application_shortcut::ApplicationShortcut;
use crate::configuration::{
    AppThemeSetting, Configuration, ConfigurationManager, ConfigurationService, Key, SettingsPath,
};
use crate::startup::auto_start::AutoStart;
use std::path::PathBuf;

pub(crate) struct SettingsState {
    pub applications: Vec<ApplicationShortcut>,
    pub theme: String,
    pub modifier: String,
    pub spotlight_modifier: String,
    pub launch_at_startup: bool,
    pub dynamic_mode: bool,
    pub pulse_border: bool,
    pub peek_mode: bool,
    pub overlay_enabled: bool,
    pub overlay_delay_ms: u16,
    pub overlay_keep_open: bool,
    pub stats_enabled: bool,
    pub spotlight_enabled: bool,
    original_configuration: Configuration,
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
        let config = manager.get().unwrap_or_default();
        Self::from_configuration(config, manager)
    }
}

impl SettingsState {
    pub fn add_running_application(&mut self, process_path: String, aumid: Option<String>) -> bool {
        let process_name = std::path::Path::new(&process_path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| process_path.clone());
        if self.applications.iter().any(|app| {
            app.process_path.eq_ignore_ascii_case(&process_path)
                || app.display_name.eq_ignore_ascii_case(&process_name)
        }) {
            return false;
        }
        self.applications.push(ApplicationShortcut {
            key: "None".to_string(),
            display_name: process_name,
            process_path,
            cycle_mode: super::application_shortcut::CycleMode::NextApp,
            start_if_not_running: true,
            aumid,
        });
        true
    }

    pub fn remove_application(&mut self, index: usize) {
        if index < self.applications.len() {
            self.applications.remove(index);
        }
    }

    pub fn move_application(&mut self, old_index: usize, new_index: usize) {
        if old_index < self.applications.len() && new_index < self.applications.len() {
            let application = self.applications.remove(old_index);
            self.applications.insert(new_index, application);
        }
    }

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
            theme: "System (follow Windows)".to_string(),
            modifier: "RightCtrl".to_string(),
            spotlight_modifier: "LeftAlt".to_string(),
            launch_at_startup: AutoStart::is_enabled().unwrap_or(false),
            dynamic_mode: true,
            pulse_border: true,
            peek_mode: false,
            overlay_enabled: false,
            overlay_delay_ms: 1000,
            overlay_keep_open: true,
            stats_enabled: false,
            spotlight_enabled: true,
            original_configuration: config.clone(),
            manager,
        };
        state.apply_configuration(config.clone());
        state.original_configuration = config;
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
        }
        .to_string();
        self.modifier = format!("{:?}", config.modifier);
        self.spotlight_modifier = format!("{:?}", config.spotlight_modifier);
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
        self.validate(&configuration)?;
        self.manager
            .update(configuration.clone())
            .map_err(|error| format!("Could not save settings: {error}"))?;
        self.original_configuration = configuration.clone();
        Ok(configuration)
    }

    pub fn reset(&mut self) -> Result<(), String> {
        let config = self
            .manager
            .load()
            .map_err(|error| format!("Could not reload settings: {error}"))?;
        self.apply_configuration(config);
        self.launch_at_startup = AutoStart::is_enabled().unwrap_or(false);
        Ok(())
    }

    pub fn is_dirty(&self) -> bool {
        self.to_configuration().map_or(true, |configuration| {
            configuration != self.original_configuration
        })
    }

    pub fn can_save(&self) -> bool {
        self.to_configuration()
            .is_ok_and(|configuration| self.is_dirty() && self.validate(&configuration).is_ok())
    }

    fn validate(&self, configuration: &Configuration) -> Result<(), String> {
        for (index, app) in configuration.applications.iter().enumerate() {
            if configuration.applications[..index]
                .iter()
                .any(|previous| previous.key == app.key)
            {
                return Err(format!(
                    "Shortcut key {:?} is assigned more than once.",
                    app.key
                ));
            }
        }
        let result = self.manager.validate(configuration);
        if result.is_success() {
            Ok(())
        } else {
            Err(result
                .message
                .unwrap_or_else(|| "Invalid configuration.".to_string()))
        }
    }

    fn to_configuration(&self) -> Result<Configuration, String> {
        let applications = self
            .applications
            .iter()
            .map(ApplicationShortcut::to_configuration)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| "Application shortcut contains an invalid key".to_string())?;
        Ok(Configuration {
            modifier: parse_modifier(&self.modifier),
            applications,
            pulse_border_enabled: self.pulse_border,
            theme: match self.theme.as_str() {
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
            spotlight_modifier: parse_modifier(&self.spotlight_modifier),
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
    use super::{ApplicationShortcut, SettingsState};
    use crate::configuration::{Configuration, ConfigurationManager, ConfigurationService};

    fn state() -> SettingsState {
        let path = std::env::temp_dir().join(format!(
            "omnispot-settings-state-{}.json",
            std::process::id()
        ));
        SettingsState::from_configuration(
            Configuration::default(),
            ConfigurationManager::new(ConfigurationService::new(path)),
        )
    }

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

    #[test]
    fn save_requires_a_valid_change_and_resets_dirty_state() {
        let mut state = state();
        assert!(!state.is_dirty());
        assert!(!state.can_save());

        state.overlay_enabled = true;
        assert!(state.is_dirty());
        assert!(state.can_save());

        state.save().unwrap();
        assert!(!state.is_dirty());
        assert!(!state.can_save());
    }

    #[test]
    fn invalid_shortcut_prevents_save() {
        let mut state = state();
        state.applications.push(ApplicationShortcut {
            key: "None".to_string(),
            display_name: "Example".to_string(),
            process_path: "example.exe".to_string(),
            cycle_mode: super::super::application_shortcut::CycleMode::NextApp,
            start_if_not_running: true,
            aumid: Some("Example!App".to_string()),
        });

        assert!(state.is_dirty());
        assert!(!state.can_save());
        assert!(state.save().is_err());
    }

    #[test]
    fn running_apps_can_be_added_deduplicated_and_reordered() {
        let mut state = state();
        assert!(state.add_running_application(r"C:\Apps\one.exe".into(), None));
        assert!(!state.add_running_application(r"C:\Apps\ONE.exe".into(), None));
        assert!(state.add_running_application(r"C:\Apps\two.exe".into(), None));

        state.move_application(0, 1);
        assert_eq!(state.applications[0].display_name, "two.exe");
        assert_eq!(state.applications.len(), 2);

        state.remove_application(0);
        assert_eq!(state.applications.len(), 1);
    }
}
