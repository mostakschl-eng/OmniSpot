use super::overlay_show_timer::OverlayShowTimer;
use super::warning_content::WarningContent;
use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(crate) struct WarningOverlayService {
    show_counts: HashMap<WarningContent, u8>,
    dismiss_timer: OverlayShowTimer,
    on_show: Arc<dyn Fn(WarningContent) + Send + Sync>,
    on_hide: Arc<dyn Fn() + Send + Sync>,
    visible: Arc<AtomicBool>,
}

impl WarningOverlayService {
    pub(crate) fn new(
        on_show: impl Fn(WarningContent) + Send + Sync + 'static,
        on_hide: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            show_counts: HashMap::new(),
            dismiss_timer: OverlayShowTimer::default(),
            on_show: Arc::new(on_show),
            on_hide: Arc::new(on_hide),
            visible: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn show(&mut self, content: WarningContent) -> bool {
        let count = self.show_counts.entry(content).or_default();
        if *count >= 3 {
            return false;
        }
        *count += 1;
        self.visible.store(true, Ordering::SeqCst);
        (self.on_show)(content);
        self.schedule_dismiss(5000);
        true
    }

    pub(crate) fn dismiss(&mut self) {
        self.dismiss_timer.cancel();
        if self.visible.swap(false, Ordering::SeqCst) {
            (self.on_hide)();
        }
    }

    fn schedule_dismiss(&mut self, delay_ms: u64) {
        let hide = self.on_hide.clone();
        let visible = self.visible.clone();
        self.dismiss_timer.configure(
            move || {
                if visible.swap(false, Ordering::SeqCst) {
                    hide();
                }
            },
            delay_ms,
        );
        self.dismiss_timer.start();
    }
}

#[cfg(test)]
mod tests {
    use super::WarningOverlayService;
    use crate::overlay::warning_content::WarningContent;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn shows_each_warning_at_most_three_times() {
        let shows = Arc::new(AtomicUsize::new(0));
        let hides = Arc::new(AtomicUsize::new(0));
        let show_count = shows.clone();
        let hide_count = hides.clone();
        let mut service = WarningOverlayService::new(
            move |_| {
                show_count.fetch_add(1, Ordering::SeqCst);
            },
            move || {
                hide_count.fetch_add(1, Ordering::SeqCst);
            },
        );
        for _ in 0..3 {
            assert!(service.show(WarningContent::Elevated));
        }
        assert!(!service.show(WarningContent::Elevated));
        service.dismiss();
        assert_eq!(shows.load(Ordering::SeqCst), 3);
        assert_eq!(hides.load(Ordering::SeqCst), 1);
        assert!(service.show(WarningContent::KeyEventsStealing));
    }
}
