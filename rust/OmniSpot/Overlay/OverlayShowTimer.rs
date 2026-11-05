use std::sync::{Arc, mpsc};
use std::time::Duration;

pub(crate) struct OverlayShowTimer {
    timeout: Duration,
    on_expired: Option<Arc<dyn Fn() + Send + Sync>>,
    cancel_sender: Option<mpsc::Sender<()>>,
}

impl Default for OverlayShowTimer {
    fn default() -> Self {
        Self {
            timeout: Duration::ZERO,
            on_expired: None,
            cancel_sender: None,
        }
    }
}

impl OverlayShowTimer {
    pub(crate) fn configure(
        &mut self,
        on_expired: impl Fn() + Send + Sync + 'static,
        timeout_ms: u64,
    ) {
        self.cancel();
        self.timeout = Duration::from_millis(timeout_ms);
        self.on_expired = Some(Arc::new(on_expired));
    }

    pub(crate) fn start(&mut self) {
        if self.cancel_sender.is_some() {
            return;
        }
        let Some(callback) = self.on_expired.clone() else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        let timeout = self.timeout;
        std::thread::spawn(move || {
            if receiver.recv_timeout(timeout).is_err() {
                callback();
            }
        });
        self.cancel_sender = Some(sender);
    }

    pub(crate) fn cancel(&mut self) {
        if let Some(sender) = self.cancel_sender.take() {
            let _ = sender.send(());
        }
    }
}

impl Drop for OverlayShowTimer {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::OverlayShowTimer;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    #[test]
    fn start_does_not_restart_and_cancel_prevents_expiration() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut timer = OverlayShowTimer::default();
        let count = calls.clone();
        timer.configure(
            move || {
                count.fetch_add(1, Ordering::SeqCst);
            },
            30,
        );
        timer.start();
        timer.start();
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        timer.cancel();
        timer.start();
        timer.cancel();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
