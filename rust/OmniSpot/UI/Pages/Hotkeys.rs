use crate::ui::SettingsWindow;
use crate::ui::controls::add_application_flyout::add_application_flyout;
use crate::ui::controls::cycle_mode_selector::cycle_mode_selector;
use crate::ui::controls::key_assignment_button::key_assignment_button;
use crate::ui::view_models::application_shortcut::ApplicationShortcut;
use gpui::{Context, FocusHandle, IntoElement, div, prelude::*, px, rgb};
use windows::{
    Win32::{
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoTaskMemFree, CoUninitialize,
        },
        UI::Shell::{
            Common::COMDLG_FILTERSPEC, FOS_FILEMUSTEXIST, FOS_PATHMUSTEXIST, FileOpenDialog,
            IFileOpenDialog, SIGDN_FILESYSPATH,
        },
    },
    core::{HRESULT, PCWSTR, w},
};

fn browse_for_executable() -> Result<Option<String>, String> {
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if initialized.is_err() {
        return Err(format!(
            "Could not initialize the file picker: {initialized}"
        ));
    }
    let result = (|| unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|error| error.to_string())?;
        let filters = [COMDLG_FILTERSPEC {
            pszName: w!("Executable files (*.exe)"),
            pszSpec: w!("*.exe"),
        }];
        dialog
            .SetFileTypes(&filters)
            .map_err(|error| error.to_string())?;
        dialog
            .SetTitle(w!("Select application"))
            .map_err(|error| error.to_string())?;
        dialog
            .SetOptions(FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST)
            .map_err(|error| error.to_string())?;
        if let Err(error) = dialog.Show(None) {
            if error.code() == HRESULT(0x800704c7u32 as i32) {
                return Ok(None);
            }
            return Err(error.to_string());
        }
        let path = dialog
            .GetResult()
            .and_then(|item| item.GetDisplayName(SIGDN_FILESYSPATH))
            .map_err(|error| error.to_string())?;
        let result = PCWSTR(path.0)
            .to_string()
            .map_err(|error| error.to_string());
        CoTaskMemFree(Some(path.0.cast()));
        result.map(Some)
    })();
    unsafe { CoUninitialize() };
    result
}

fn app_row(
    app: &ApplicationShortcut,
    index: usize,
    listening: bool,
    focus_handle: FocusHandle,
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
                .child(key_assignment_button(
                    &app.key,
                    index,
                    listening,
                    focus_handle,
                    cx,
                ))
                .child(div().flex_1().child(app.display_name.clone()))
                .child(
                    div()
                        .id(("move-app-up", index))
                        .px_2()
                        .py_1()
                        .cursor_pointer()
                        .child("↑")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if index > 0 {
                                this.state.move_application(index, index - 1);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id(("move-app-down", index))
                        .px_2()
                        .py_1()
                        .cursor_pointer()
                        .child("↓")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if index + 1 < this.state.applications.len() {
                                this.state.move_application(index, index + 1);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .id(("remove-app", index))
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .cursor_pointer()
                        .child("Remove")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state.remove_application(index);
                            cx.notify();
                        })),
                ),
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
                .child(
                    div()
                        .id(("start-if-not-running", index))
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .cursor_pointer()
                        .child(if app.start_if_not_running {
                            "Start if not running: On"
                        } else {
                            "Start if not running: Off"
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(app) = this.state.applications.get_mut(index) {
                                app.start_if_not_running = !app.start_if_not_running;
                            }
                            cx.notify();
                        })),
                ),
        )
}

impl SettingsWindow {
    pub(crate) fn applications(
        &self,
        cx: &mut Context<Self>,
        focus_handle: FocusHandle,
    ) -> impl IntoElement {
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
                                if !this.add_flyout_open {
                                    this.available_apps = crate::ui::windows::settings::running_applications();
                                }
                                this.add_flyout_open = !this.add_flyout_open;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.add_flyout_open, |element| {
                element.child(add_application_flyout()).child(
                    div()
                        .flex()
                        .flex_col()
                        .children(
                            self.available_apps
                                .iter()
                                .enumerate()
                                .map(|(index, (name, path, aumid))| {
                                    let path = path.clone();
                                    let aumid = aumid.clone();
                                    div()
                                        .id(("available-app", index))
                                        .px_3()
                                        .py_2()
                                        .rounded_md()
                                        .hover(|style| style.bg(rgb(0x414744)))
                                        .cursor_pointer()
                                        .child(name.clone())
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if !this.state.add_running_application(path.clone(), aumid.clone()) {
                                                this.save_status = Some(("This application is already in the list.".to_string(), false));
                                            }
                                            this.add_flyout_open = false;
                                            cx.notify();
                                        }))
                                }),
                        )
                        .when(self.available_apps.is_empty(), |element| {
                            element.child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .text_color(rgb(0xc1c4c2))
                                    .child("No running applications found."),
                            )
                        })
                        .child(
                            div()
                                .id("browse-application")
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(rgb(0x414744))
                                .cursor_pointer()
                                .child("Browse for executable")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    match browse_for_executable() {
                                        Ok(Some(path)) => {
                                            if !crate::window_discovery::process_inspector::ProcessInspector::is_windows_executable(&path) {
                                                this.save_status = Some(("The selected file is not a valid Windows executable.".to_string(), false));
                                            } else if !this.state.add_running_application(path, None) {
                                                this.save_status = Some(("This application is already in the list.".to_string(), false));
                                            } else {
                                                this.add_flyout_open = false;
                                            }
                                        }
                                        Ok(None) => {}
                                        Err(error) => this.save_status = Some((format!("Could not browse for an executable: {error}"), false)),
                                    }
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(
                div()
                    .id("applications-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(self.state.applications.iter().enumerate().map(|(index, app)| app_row(app, index, self.assigning_key == Some(index), focus_handle.clone(), cx).into_any_element()).collect::<Vec<_>>())
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
