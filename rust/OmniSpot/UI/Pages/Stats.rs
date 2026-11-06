use crate::ui::SettingsWindow;
use crate::ui::controls::bento_dashboard::bento_dashboard;
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};
use windows::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNO, MessageBoxW,
};
use windows::core::w;

fn set_stats_enabled(this: &mut SettingsWindow, enabled: bool) {
    this.state.stats_enabled = enabled;
    match this.state.save() {
        Ok(configuration) => {
            if let Some(events) = &this.events {
                let _ = events.try_send(crate::input::hook::InputEvent::ConfigurationUpdated(
                    configuration,
                ));
            }
            this.save_status = Some(("Statistics setting saved.".into(), true));
        }
        Err(error) => this.save_status = Some((error, false)),
    }
}

fn remove_stale_shortcut(this: &mut SettingsWindow, process_name: &str) {
    let index = this.state.applications.iter().position(|app| {
        std::path::Path::new(&app.process_path)
            .file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(process_name))
    });
    let Some(index) = index else { return };
    this.state.remove_application(index);
    match this.state.save() {
        Ok(configuration) => {
            if let Some(events) = &this.events {
                let _ = events.try_send(crate::input::hook::InputEvent::ConfigurationUpdated(
                    configuration,
                ));
            }
            this.save_status = Some((format!("Removed unused shortcut for {process_name}."), true));
        }
        Err(error) => this.save_status = Some((error, false)),
    }
}

impl SettingsWindow {
    pub(crate) fn stats(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let shield = div()
            .w_full()
            .h(px(440.0))
            .rounded_lg()
            .bg(rgb(0x2b302d))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(div().text_2xl().child("Privacy Shield"))
            .child(
                div()
                    .text_color(rgb(0xc1c4c2))
                    .child("Usage statistics are off by default"),
            )
            .child(
                div()
                    .w(px(380.0))
                    .p_4()
                    .rounded_md()
                    .bg(rgb(0x363b38))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child("Privacy Pledge")
                    .child("All usage data is stored locally on your device only.")
                    .child("Nothing is ever transmitted, sold, or shared.")
                    .child("You can disable collection at any time."),
            )
            .child(
                div()
                    .id("enable-statistics")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0xc6b690))
                    .text_color(rgb(0x171a18))
                    .cursor_pointer()
                    .child("Enable Statistics")
                    .on_click(cx.listener(|this, _, _, cx| {
                        set_stats_enabled(this, true);
                        cx.notify();
                    })),
            );

        div()
            .id("stats-scroll")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .child("Statistics")
            .child(if self.state.stats_enabled {
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .id("disable-statistics")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x414744))
                            .cursor_pointer()
                            .child("Disable Data Collection")
                            .on_click(cx.listener(|this, _, _, cx| {
                                set_stats_enabled(this, false);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("reset-statistics")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x414744))
                            .cursor_pointer()
                            .child("Reset all statistics")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let confirmed = unsafe {
                                    MessageBoxW(
                                        None,
                                        w!("This permanently deletes all usage statistics."),
                                        w!("Reset all statistics?"),
                                        MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
                                    ) == IDYES
                                };
                                if confirmed {
                                    this.save_status = this.stats.as_ref().map(|stats| match stats
                                        .lock()
                                    {
                                        Ok(mut stats) => match stats.reset() {
                                            Ok(()) => ("Statistics were reset.".into(), true),
                                            Err(error) => (error, false),
                                        },
                                        Err(_) => {
                                            ("Statistics are currently unavailable.".into(), false)
                                        }
                                    });
                                }
                                cx.notify();
                            })),
                    )
                    .child({
                        let configured = self
                            .state
                            .applications
                            .iter()
                            .filter(|app| app.key != "None")
                            .map(|app| {
                                let process = std::path::Path::new(&app.process_path)
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| app.process_path.clone());
                                (process, app.key.clone(), app.display_name.clone())
                            })
                            .collect::<Vec<_>>();
                        let metrics = self.stats.as_ref().and_then(|stats| {
                            stats.lock().ok().map(|stats| stats.metrics(&configured))
                        });
                        if let Some(metrics) = metrics {
                            let stale = metrics.first_stale_shortcut.clone();
                            let remove_button = stale.map(|shortcut| {
                                let process_name = shortcut.process_name;
                                div()
                                    .id("remove-stale-shortcut")
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .bg(rgb(0x414744))
                                    .cursor_pointer()
                                    .child(format!("Remove unused '{}' shortcut", shortcut.letter))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        remove_stale_shortcut(this, &process_name);
                                        cx.notify();
                                    }))
                            });
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(bento_dashboard(&metrics))
                                .when_some(remove_button, |element, button| element.child(button))
                                .into_any_element()
                        } else {
                            div()
                                .child("Statistics will appear after the first recorded action")
                                .into_any_element()
                        }
                    })
                    .into_any_element()
            } else {
                shield.into_any_element()
            })
    }
}
