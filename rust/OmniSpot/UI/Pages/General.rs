use crate::ui::SettingsWindow;
use crate::ui::controls::modifier_key_selector::modifier_key_selector;
use crate::ui::controls::settings_card::settings_card;
use gpui::{Context, IntoElement, div, prelude::*, rgb};

pub(crate) fn pulse_border_supported() -> bool {
    use windows::Win32::System::SystemInformation::{GetVersionExW, OSVERSIONINFOW};
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    (unsafe { GetVersionExW(&mut version).is_ok() })
        && supports_pulse_border(version.dwMajorVersion, version.dwBuildNumber)
}

fn supports_pulse_border(major: u32, build: u32) -> bool {
    major >= 10 && (major > 10 || build >= 22_000)
}

impl SettingsWindow {
    pub(crate) fn general(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("general-scroll")
            .w_full()
            .h_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .child("Appearance")
            .child(settings_card(
                "Theme",
                "Choose the application color theme.",
                div()
                    .id("theme-select")
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x3b403e))
                    .cursor_pointer()
                    .child(format!("{}  ▾", self.state.theme))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.theme_menu_open = !this.theme_menu_open;
                        cx.notify();
                    })),
            ))
            .when(self.theme_menu_open, |element| {
                element.child(
                    div().flex().gap_2().children(
                        ["System (follow Windows)", "Dark", "Light"].map(|theme| {
                            div()
                                .id(theme)
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(rgb(0x3b403e))
                                .cursor_pointer()
                                .child(theme)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.state.theme = theme.to_string();
                                    this.theme_menu_open = false;
                                    cx.notify();
                                }))
                        }),
                    ),
                )
            })
            .child("Modifier Key")
            .child(modifier_key_selector(&self.state.modifier, false, cx))
            .child("Behavior")
            .child(settings_card(
                "Launch at startup",
                "Start OmniSpot automatically when Windows boots",
                self.toggle(0, cx),
            ))
            .child(settings_card(
                "Dynamic Mode",
                "Automatically assigns hotkey letters to running apps based on their name. Letters already assigned to a static shortcut are not affected.",
                self.toggle(1, cx),
            ))
            .child(settings_card(
                "Pulse border effect",
                "Briefly highlights the window border with a gold pulse when focusing new window. Requires Windows 11 (build 22000+).",
                self.toggle(2, cx),
            ))
            .child("Peek mode")
            .child(settings_card(
                "Peek mode",
                "Hold the hotkey (modifier + letter) for at least 400 ms to activate. Release the modifier to automatically snap back to your previous window.",
                self.toggle(3, cx),
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::supports_pulse_border;

    #[test]
    fn pulse_border_requires_windows_11_build_22000() {
        assert!(!supports_pulse_border(10, 21_999));
        assert!(supports_pulse_border(10, 22_000));
        assert!(supports_pulse_border(11, 22_000));
        assert!(!supports_pulse_border(6, 22_000));
    }
}
