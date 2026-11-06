use crate::{
    configuration::{ConfigurationManager, ConfigurationService, SettingsPath},
    spotlight::{SearchResult as CoreSearchResult, SpotlightSearchService, SpotlightUsageStore},
    ui::view_models::spotlight::{SearchResult, SpotlightState},
    window_discovery::WindowEnumerator,
};
use gpui::{Context, IntoElement, KeyDownEvent, Render, Window, div, img, prelude::*, px, rgb};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use windows::{
    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    core::{PCWSTR, w},
};

pub struct SpotlightWindow {
    pub state: SpotlightState,
    service: SpotlightSearchService,
    configuration: crate::configuration::Configuration,
    windows: WindowEnumerator,
    stats: Option<Arc<Mutex<crate::stats::StatsService>>>,
    query_generation: u64,
    file_search_task: Option<gpui::Task<()>>,
}

impl Default for SpotlightWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl SpotlightWindow {
    pub fn new() -> Self {
        Self::with_stats(None)
    }

    pub fn with_stats(stats: Option<Arc<Mutex<crate::stats::StatsService>>>) -> Self {
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
            stats,
            query_generation: 0,
            file_search_task: None,
        };
        let recent = result.recent_process_names();
        result.state.suggestions = result
            .service
            .suggestions(&result.configuration, &mut result.windows, &recent)
            .into_iter()
            .map(to_ui_result)
            .collect();
        result
    }

    fn recent_process_names(&self) -> Vec<String> {
        self.stats
            .as_ref()
            .and_then(|stats| stats.lock().ok().map(|stats| stats.recent_process_names()))
            .unwrap_or_default()
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.file_search_task = None;
        let query = self.state.query.clone();
        let recent_process_names = self.recent_process_names();
        let results = if query.trim().is_empty() {
            self.service.suggestions(
                &self.configuration,
                &mut self.windows,
                &recent_process_names,
            )
        } else {
            self.service.search_apps(
                &query,
                &self.configuration,
                &mut self.windows,
                &recent_process_names,
            )
        }
        .into_iter()
        .map(to_ui_result)
        .collect::<Vec<_>>();
        self.query_generation = self.query_generation.wrapping_add(1);
        let generation = self.query_generation;
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

        if query.trim().len() >= 2 {
            let entity = cx.entity();
            let search_query = query.clone();
            let background = cx.background_executor().clone();
            let delay = background.timer(std::time::Duration::from_millis(25));
            let file_search = background.spawn(async move {
                delay.await;
                crate::spotlight::FileSearchProvider::default().search(&search_query, 15)
            });
            self.file_search_task =
                Some(cx.spawn(async move |_, cx| {
                    let files = file_search
                        .await
                        .into_iter()
                        .map(to_ui_result)
                        .collect::<Vec<_>>();
                    let _ =
                        entity.update(cx, |this, cx| {
                            if this.query_generation != generation || this.state.query != query {
                                return;
                            }
                            for file in files {
                                if !this.state.results.iter().any(|existing| {
                                    existing.target.eq_ignore_ascii_case(&file.target)
                                }) {
                                    this.state.results.push(file);
                                }
                            }
                            this.state.selected_path = this
                                .active_results()
                                .get(this.state.selected_index)
                                .map(|result| result.target.clone())
                                .unwrap_or_default();
                            cx.notify();
                        });
                }));
        }
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
            .on_click(cx.listener(move |this, _, window, cx| {
                this.state.selected_index = index;
                this.open_selected();
                window.remove_window();
                cx.notify();
            }))
    }

    fn open_selected(&mut self) {
        let Some(result) = self.active_results().get(self.state.selected_index) else {
            return;
        };
        let result = result.clone();
        let target = result.target.clone();
        let open_result = unsafe {
            if target.starts_with("shell:AppsFolder\\") {
                let target_wide = target.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
                ShellExecuteW(
                    None,
                    w!("open"),
                    w!("explorer.exe"),
                    PCWSTR(target_wide.as_ptr()),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                )
            } else {
                let target_wide = target.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
                ShellExecuteW(
                    None,
                    w!("open"),
                    PCWSTR(target_wide.as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                )
            }
        };
        if open_result.0 as isize <= 32 {
            crate::logging::debug(format!("Could not open Spotlight result: {target}"));
            return;
        }
        let core = match result.kind.as_str() {
            "FILE" => CoreSearchResult::file(
                result.title,
                result.target.clone(),
                result.target,
                0,
                result.icon_path,
            ),
            _ => CoreSearchResult::app(&result.title, &result.target, &target, 0),
        };
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
                let query = this.state.query.clone();
                if this.state.handle_key(&key, typed) {
                    if key == "enter" {
                        this.open_selected();
                        window.remove_window();
                    } else if key != "escape" {
                        if this.state.query != query {
                            this.refresh(cx);
                        }
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
