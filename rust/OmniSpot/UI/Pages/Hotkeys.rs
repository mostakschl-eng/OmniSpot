use crate::ui::SettingsWindow;
use crate::ui::controls::add_application_flyout::add_application_flyout;
use crate::ui::controls::cycle_mode_selector::cycle_mode_selector;
use crate::ui::controls::key_assignment_button::key_assignment_button;
use crate::ui::view_models::application_shortcut::ApplicationShortcut;
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};

fn app_row(
    app: &ApplicationShortcut,
    index: usize,
    cx: &mut Context<SettingsWindow>,
) -> impl IntoElement {
    div()
        .w_full()
        .p_3()
        .rounded_md()
        .bg(rgb(0x303532))
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(key_assignment_button(&app.key))
                .child(div().flex_1().child(app.display_name.clone()))
                .child("⋮"),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0xc1c4c2))
                .child(app.process_path.clone()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child("Cycle mode")
                .child(cycle_mode_selector(&app.cycle_mode, index, cx))
                .child(if app.start_if_not_running {
                    "Start if not running: On"
                } else {
                    "Start if not running: Off"
                }),
        )
}

impl SettingsWindow {
    pub(crate) fn applications(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .h(px(40.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child("Applications")
                    .child(
                        div()
                            .id("add-application")
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(0x383d3a))
                            .cursor_pointer()
                            .child("+  Add")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.add_flyout_open = !this.add_flyout_open;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.add_flyout_open, |element| element.child(add_application_flyout()))
            .child(
                div()
                    .id("applications-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(self.state.applications.iter().enumerate().map(|(index, app)| app_row(app, index, cx).into_any_element()).collect::<Vec<_>>())
                    .when(self.state.applications.is_empty(), |element| {
                        element.child(
                            div()
                                .h_full()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap_3()
                                .child("No applications configured")
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(0xc1c4c2))
                                        .child("Add an application and assign a key. Press your modifier, then that key to switch to it."),
                                ),
                        )
                    }),
            )
    }
}
