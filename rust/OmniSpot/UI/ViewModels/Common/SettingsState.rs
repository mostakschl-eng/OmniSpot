use super::application_shortcut::ApplicationShortcut;

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
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            applications: Vec::new(),
            theme: "System (follow Windows)",
            modifier: "RightCtrl",
            spotlight_modifier: "LeftAlt",
            launch_at_startup: false,
            dynamic_mode: true,
            pulse_border: true,
            peek_mode: false,
            overlay_enabled: false,
            overlay_delay_ms: 1000,
            overlay_keep_open: true,
            stats_enabled: false,
            spotlight_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsState;

    #[test]
    fn ui_defaults_match_new_csharp_profile() {
        let state = SettingsState::default();
        assert_eq!(state.modifier, "RightCtrl");
        assert_eq!(state.spotlight_modifier, "LeftAlt");
        assert_eq!(state.overlay_delay_ms, 1000);
        assert!(state.dynamic_mode);
        assert!(state.pulse_border);
        assert!(!state.stats_enabled);
    }
}
