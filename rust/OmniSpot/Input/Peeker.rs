use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Sender},
};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) const PEEK_THRESHOLD_MS: u64 = 400;

#[derive(Debug, Clone)]
pub(crate) struct PeekResult {
    pub previous_handle: isize,
    pub target_handle: isize,
    pub target_process: String,
    pub target_was_minimized: bool,
    pub dynamic: bool,
    pub armed_at: Instant,
}

struct ArmedPeek {
    cancel: Sender<()>,
    active: Arc<AtomicBool>,
    previous_handle: isize,
    target_handle: isize,
    target_process: String,
    target_was_minimized: bool,
    dynamic: bool,
    armed_at: Instant,
}

#[derive(Default)]
pub(crate) struct Peeker {
    armed: Option<ArmedPeek>,
}

impl Peeker {
    pub(crate) fn arm(
        &mut self,
        previous_handle: isize,
        target_handle: isize,
        target_process: String,
        target_was_minimized: bool,
        dynamic: bool,
    ) {
        self.cancel();
        let (cancel, receiver) = mpsc::channel();
        let active = Arc::new(AtomicBool::new(false));
        let timer_active = Arc::clone(&active);
        thread::spawn(move || {
            if receiver
                .recv_timeout(Duration::from_millis(PEEK_THRESHOLD_MS))
                .is_err()
            {
                timer_active.store(true, Ordering::Release);
            }
        });
        self.armed = Some(ArmedPeek {
            cancel,
            active,
            previous_handle,
            target_handle,
            target_process,
            target_was_minimized,
            dynamic,
            armed_at: Instant::now(),
        });
    }

    pub(crate) fn finish(&mut self) -> Option<PeekResult> {
        let armed = self.armed.take()?;
        let active = armed.active.load(Ordering::Acquire);
        let _ = armed.cancel.send(());
        active.then_some(PeekResult {
            previous_handle: armed.previous_handle,
            target_handle: armed.target_handle,
            target_process: armed.target_process,
            target_was_minimized: armed.target_was_minimized,
            dynamic: armed.dynamic,
            armed_at: armed.armed_at,
        })
    }

    pub(crate) fn cancel(&mut self) {
        if let Some(armed) = self.armed.take() {
            let _ = armed.cancel.send(());
        }
    }
}

impl Drop for Peeker {
    fn drop(&mut self) {
        self.cancel();
    }
}
