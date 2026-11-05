use crate::{
    configuration::{ConfigurationManager, ConfigurationService, SettingsPath},
    spotlight::{SearchResult as CoreSearchResult, SpotlightSearchService, SpotlightUsageStore},
    ui::view_models::spotlight::{SearchResult, SpotlightState},
    window_discovery::WindowEnumerator,
};
use gpui::{Context, IntoElement, KeyDownEvent, Render, Window, div, img, prelude::*, px, rgb};
use std::{path::PathBuf, process::Command};

pub struct SpotlightWindow {
    pub state: SpotlightState,
    service: SpotlightSearchService,
    configuration: crate::configuration::Configuration,
    windows: WindowEnumerator,
}

impl Default for SpotlightWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl SpotlightWindow {
    pub fn new() -> Self {
        let executable_dir = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."));
        let portable = executable_dir.join(".portable").exists();
        let settings_path = SettingsPath::resolve(&executable_dir, portable)
            .unwrap_or_else(|_| PathBuf::from("settings.json"));
        let mut manager = ConfigurationManager::new(ConfigurationService::new(settings_path));
        let configuration = manager.load().unwrap_or_default();
        let usage_path = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("OmniSpot")
            .join("spotlight_usage.json");
        let service = SpotlightSearchService::new(SpotlightUsageStore::new(usage_path));
        let mut result = Self {
            state: SpotlightState::default(),
            service,
            configuration,
            windows: WindowEnumerator::default(),
        };
        result.refresh();
        result
    }

    fn refresh(&mut self) {
        let query = self.state.query.clone();
        let results = self
            .service
            .search(&query, &self.configuration, &mut self.windows)
            .into_iter()
            .map(to_ui_result)
            .collect::<Vec<_>>();
        if query.is_empty() {
            self.state.suggestions = results;
            self.state.results.clear();
        } else {
            self.state.results = results;
            self.state.suggestions.clear();
        }
        self.state.selected_index = 0;
        self.state.selected_path = self
            .active_results()
            .first()
            .map(|result| result.target.clone())
            .unwrap_or_default();
    }

    fn active_results(&self) -> &[SearchResult] {
        if self.state.query.is_empty() {
            &self.state.suggestions
        } else {
            &self.state.results
        }
    }

    fn result_row(
        result: SearchResult,
        selected: bool,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div()
            .id(("spotlight-result", index))
            .h(px(42.0))
            .px_3()
            .rounded_md()
            .bg(rgb(if selected { 0x303a46 } else { 0x191d24 }))
            .hover(|style| style.bg(rgb(0x303a46)))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_between()
            .child(if let Some(icon_path) = &result.icon_path {
                img(PathBuf::from(icon_path))
                    .size(px(20.0))
                    .into_any_element()
            } else {
                div()
                    .size(px(20.0))
                    .rounded_sm()
                    .bg(rgb(0x414744))
                    .into_any_element()
            })
            .child(result.title.clone())
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0xabb3bb))
                    .child(result.kind.clone()),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.state.selected_index = index;
                this.open_selected();
                cx.notify();
            }))
    }

    fn open_selected(&mut self) {
        let Some(result) = self.active_results().get(self.state.selected_index) else {
            return;
        };
        let target = result.target.clone();
        let core = CoreSearchResult::app(&result.title, &result.target, &target, 0);
        let _ = Command::new("cmd")
            .args(["/C", "start", "", target.as_str()])
            .spawn();
        self.service.record_launch(&core);
    }
}

fn to_ui_result(result: CoreSearchResult) -> SearchResult {
    SearchResult {
        title: result.title,
        kind: match result.result_type {
            crate::spotlight::SearchResultType::App => "APP".to_string(),
            crate::spotlight::SearchResultType::File => "FILE".to_string(),
        },
        target: result.target,
        icon_path: result.icon_path,
    }
}

impl Render for SpotlightWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = if self.state.query.is_empty() {
            &self.state.suggestions
        } else {
            &self.state.results
        };
        let mut rows = Vec::with_capacity(active.len());
        for (index, result) in active.iter().enumerate() {
            rows.push(Self::result_row(
                result.clone(),
                index == self.state.selected_index,
                index,
                cx,
            ));
        }
        div()
            .size_full()
            .p_3()
            .rounded_lg()
            .bg(rgb(0x191d24))
            .text_color(rgb(0xf4f7fb))
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let typed = event.keystroke.key_char.as_deref();
                let key = event.keystroke.key.to_ascii_lowercase();
                if this.state.handle_key(&key, typed) {
                    if key == "enter" {
                        this.open_selected();
                        window.remove_window();
                    } else if key != "escape" {
                        this.refresh();
                    } else {
                        window.remove_window();
                    }
                    cx.notify();
                }
            }))
            .child(
                div()
                    .h(px(56.0))
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .h(px(44.0))
                            .px_4()
                            .rounded_md()
                            .bg(rgb(0x242a33))
                            .flex()
                            .items_center()
                            .child(if self.state.query.is_empty() {
                                "⌕   Search apps and files".to_owned()
                            } else {
                                self.state.query.clone()
                            }),
                    )
                    .child("ESC"),
            )
            .child(div().h(px(1.0)).bg(rgb(0x43505a)))
            .child(
                div()
                    .flex_1()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_xs().child(if self.state.query.is_empty() {
                        "QUICK ACCESS"
                    } else {
                        "SEARCH RESULTS"
                    }))
                    .children(rows),
            )
            .child(div().h(px(1.0)).bg(rgb(0x43505a)))
            .child(
                div()
                    .h(px(42.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .child(self.state.selected_path.clone())
                    .child("↑↓  Select     Enter  Open"),
            )
    }
}
