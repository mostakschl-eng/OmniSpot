use super::efficiency_calculator;
use super::stats_event::StatsEvent;
use super::{IAppRegistryCache, ISessionStats};

fn get_performance_frequency() -> i64 {
    #[cfg(windows)]
    {
        let mut freq = 0i64;
        unsafe {
            let _ = windows::Win32::System::Performance::QueryPerformanceFrequency(&mut freq);
        }
        if freq > 0 {
            freq
        } else {
            10_000_000
        }
    }
    #[cfg(not(windows))]
    {
        10_000_000
    }
}

pub(crate) fn elapsed_ms(start_tick: i64, end_tick: i64, frequency: i64) -> i32 {
    let diff = (end_tick - start_tick).max(0);
    ((diff as f64 * 1000.0) / (frequency as f64)) as i32
}

pub(crate) struct StatsConsumer<S: ISessionStats, C: IAppRegistryCache, F: Fn(&str)> {
    session_stats: S,
    #[allow(dead_code)]
    app_registry_cache: C,
    flush: F,
    previous_process_name: Option<String>,
    frequency: i64,
}

impl<S: ISessionStats, C: IAppRegistryCache, F: Fn(&str)> StatsConsumer<S, C, F> {
    pub(crate) const IDLE_THRESHOLD_MS: i32 = 1500;
    pub(crate) const BASELINE_DURATION_MS: i32 = 350;

    pub(crate) fn new(session_stats: S, app_registry_cache: C, flush: F) -> Self {
        Self {
            session_stats,
            app_registry_cache,
            flush,
            previous_process_name: None,
            frequency: get_performance_frequency(),
        }
    }

    pub(crate) fn set_frequency(&mut self, frequency: i64) {
        self.frequency = frequency;
    }

    pub(crate) fn process_event(&mut self, event: StatsEvent) {
        match event {
            StatsEvent::SwitchEvent {
                process_name,
                process_id: _,
                process_path: _,
                total_choices,
                modifier_down_tick,
                letter_down_tick,
                previous_letter_up_tick,
                is_dynamic,
                trigger_key,
            } => {
                let raw_duration_ms = if let Some(prev_up) = previous_letter_up_tick {
                    elapsed_ms(prev_up, letter_down_tick, self.frequency)
                } else {
                    elapsed_ms(modifier_down_tick, letter_down_tick, self.frequency)
                };

                let duration_ms = if raw_duration_ms > Self::IDLE_THRESHOLD_MS {
                    Self::BASELINE_DURATION_MS
                } else {
                    raw_duration_ms
                };

                let saved_ms = efficiency_calculator::saved_ms(total_choices, duration_ms);
                let fastest_duration_ms = if raw_duration_ms <= Self::IDLE_THRESHOLD_MS {
                    Some(raw_duration_ms)
                } else {
                    None
                };

                let trigger_key_str = format!("{}", trigger_key);
                self.session_stats.record_switch(
                    &process_name,
                    self.previous_process_name.as_deref(),
                    duration_ms,
                    saved_ms,
                    is_dynamic,
                    fastest_duration_ms,
                    &trigger_key_str,
                );
                self.previous_process_name = Some(process_name);
            }
            StatsEvent::PeekEvent {
                target_process_name,
                target_process_path: _,
                arm_tick,
                finish_tick,
                is_dynamic,
            } => {
                let duration_ms = elapsed_ms(arm_tick, finish_tick, self.frequency);
                self.session_stats
                    .record_peek(&target_process_name, duration_ms, is_dynamic);
            }
            StatsEvent::AltTabEvent(nav_count) => {
                self.session_stats.record_alt_tab(nav_count);
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn try_flush(&self, reason: &str) {
        (self.flush)(reason);
    }

    pub(crate) fn session_stats(&self) -> &S {
        &self.session_stats
    }

    pub(crate) fn session_stats_mut(&mut self) -> &mut S {
        &mut self.session_stats
    }
}

#[cfg(test)]
#[path = "../tests/Stats/StatsConsumerTests.rs"]
mod tests;
