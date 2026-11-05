use crate::ui::view_models::application_shortcut::CycleMode;

pub(crate) fn cycle_mode_to_label(mode: &CycleMode) -> &'static str {
    match mode {
        CycleMode::NextApp => "Next App",
        CycleMode::Hide => "Hide",
        CycleMode::NextWindow => "Next Window",
    }
}

#[cfg(test)]
mod tests {
    use super::cycle_mode_to_label;
    use crate::ui::view_models::application_shortcut::CycleMode;

    #[test]
    fn labels_match_csharp_ui() {
        assert_eq!(cycle_mode_to_label(&CycleMode::NextApp), "Next App");
        assert_eq!(cycle_mode_to_label(&CycleMode::Hide), "Hide");
        assert_eq!(cycle_mode_to_label(&CycleMode::NextWindow), "Next Window");
    }
}
