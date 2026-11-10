use super::application_window::ApplicationWindow;
use super::window_enumerator::FocusedAppWindows;
use crate::configuration::configuration::ApplicationConfiguration;

pub(crate) trait IWindowEnumerator {
    fn get_windows(&self) -> Vec<ApplicationWindow>;
    fn get_cached_windows(&mut self) -> Vec<ApplicationWindow>;
    fn get_current_window(&self) -> Option<ApplicationWindow>;
    fn get_focused_app_windows(
        &self,
        all: &[ApplicationWindow],
        applications: &[ApplicationConfiguration],
    ) -> FocusedAppWindows;
    fn get_all_windows(&self) -> Vec<ApplicationWindow>;

    fn get_total_choices_count(&mut self) -> usize {
        self.get_cached_windows().len()
    }

    fn log_windows(&self, level: i32, windows: &[ApplicationWindow]) {
        for window in windows {
            super::log(
                level,
                format!(
                    "PID/Handle: {}/{}, {}, {:?}",
                    window.process_id, window.handle, window.process_image_path, window.title
                ),
            );
        }
    }

    fn log_all_windows(&self) {
        self.log_windows(2, &self.get_all_windows());
    }
}
