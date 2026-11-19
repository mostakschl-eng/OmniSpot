use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::configuration::configuration::Configuration;
use super::app_registry_cache::AppRegistryCache;
use super::migrations::stats_migration_runner::StatsMigrationRunner;
use super::session_stats::{system_today, SessionStats};
use super::stats_consumer::StatsConsumer;
use super::stats_event::StatsEvent;
use super::storage::{DailyBucketDocument, StatsDatabase};

pub(crate) struct StatsService {
    session_stats: Arc<SessionStats>,
    registry_cache: Arc<AppRegistryCache>,
    db: Arc<Mutex<StatsDatabase>>,
    db_exists: Arc<AtomicBool>,
    sender: Mutex<Option<SyncSender<StatsEvent>>>,
    worker_handle: Mutex<Option<JoinHandle<()>>>,
    is_running: Arc<AtomicBool>,
    flush_lock: Mutex<()>,
}

impl StatsService {
    pub(crate) fn new(
        session_stats: Arc<SessionStats>,
        registry_cache: Arc<AppRegistryCache>,
        db: Arc<Mutex<StatsDatabase>>,
        db_exists: bool,
    ) -> Self {
        Self {
            session_stats,
            registry_cache,
            db,
            db_exists: Arc::new(AtomicBool::new(db_exists)),
            sender: Mutex::new(None),
            worker_handle: Mutex::new(None),
            is_running: Arc::new(AtomicBool::new(false)),
            flush_lock: Mutex::new(()),
        }
    }

    pub(crate) fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub(crate) fn start(&self, stats_enabled: bool) {
        if stats_enabled {
            self.start_consumer();
        }
    }

    pub(crate) fn update_configuration(&self, config: &Configuration) {
        let should_run = config.stats_enabled;
        let running = self.is_running();

        if should_run && !running {
            self.start_consumer();
        } else if !should_run && running {
            self.stop_consumer("stats disabled");
        }
    }

    pub(crate) fn enqueue(&self, event: StatsEvent) {
        if !self.is_running() {
            return;
        }

        let sender_guard = self.sender.lock().unwrap();
        if let Some(sender) = sender_guard.as_ref() {
            match sender.try_send(event) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    // Channel bounded limit reached - drop oldest / drop event
                }
                Err(TrySendError::Disconnected(_)) => {}
            }
        }
    }

    pub(crate) fn flush(&self, _reason: &str) {
        if !self.is_running() {
            return;
        }

        let _lock = self.flush_lock.lock().unwrap();
        let snapshots = self.session_stats.get_snapshots();

        if let Ok(mut db) = self.db.lock() {
            for snapshot in &snapshots {
                if snapshot.date.is_empty() {
                    continue;
                }

                if let Ok(val) = serde_json::to_value(snapshot) {
                    db.daily_buckets.insert(snapshot.date.clone(), val);
                }
            }

            if snapshots.len() > 1 {
                self.session_stats.clear_rollover();
            }
        }
    }

    pub(crate) fn load_todays_bucket(&self) {
        if !self.db_exists.load(Ordering::SeqCst) {
            return;
        }

        let today = system_today();
        if let Ok(db) = self.db.lock() {
            if let Some(val) = db.daily_buckets.get(&today) {
                if let Ok(doc) = serde_json::from_value::<DailyBucketDocument>(val.clone()) {
                    self.session_stats.load_from(&doc);
                }
            }
        }
    }

    fn start_consumer(&self) {
        if self.is_running() {
            return;
        }

        // Run pending migrations
        let runner = StatsMigrationRunner::new(
            self.db_exists.load(Ordering::SeqCst),
            super::migrations::all_migrations(),
        );

        if let Ok(mut db) = self.db.lock() {
            if !runner.run_pending(&mut db) {
                return;
            }
        }

        self.load_todays_bucket();

        let (tx, rx) = sync_channel::<StatsEvent>(1000);
        *self.sender.lock().unwrap() = Some(tx);
        self.is_running.store(true, Ordering::SeqCst);

        let running_flag = Arc::clone(&self.is_running);
        let session_stats = Arc::clone(&self.session_stats);
        let registry_cache = Arc::clone(&self.registry_cache);
        let db = Arc::clone(&self.db);

        let handle = thread::spawn(move || {
            worker_loop(rx, running_flag, session_stats, registry_cache, db);
        });

        *self.worker_handle.lock().unwrap() = Some(handle);
    }

    pub(crate) fn stop_consumer(&self, reason: &str) {
        if !self.is_running() {
            return;
        }

        self.flush(reason);
        self.is_running.store(false, Ordering::SeqCst);

        // Drop sender so worker wakes up and exits
        let _ = self.sender.lock().unwrap().take();

        let handle = self.worker_handle.lock().unwrap().take();
        if let Some(h) = handle {
            let _ = h.join();
        }
    }
}

impl Drop for StatsService {
    fn drop(&mut self) {
        if self.is_running() {
            self.stop_consumer("app exit");
        }
    }
}

fn worker_loop(
    rx: Receiver<StatsEvent>,
    running: Arc<AtomicBool>,
    session_stats: Arc<SessionStats>,
    registry_cache: Arc<AppRegistryCache>,
    db: Arc<Mutex<StatsDatabase>>,
) {
    let flush_stats = Arc::clone(&session_stats);
    let flush_db = Arc::clone(&db);
    let mut consumer = StatsConsumer::new(
        session_stats.clone(),
        registry_cache.clone(),
        move |_reason| {
            do_flush(&flush_stats, &flush_db);
        },
    );
    let mut last_flush = std::time::Instant::now();
    let flush_interval = Duration::from_secs(300); // 5 minutes

    while running.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => {
                consumer.process_event(event);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Check if periodic flush needed
                if last_flush.elapsed() >= flush_interval {
                    do_flush(&session_stats, &db);
                    last_flush = std::time::Instant::now();
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    // Flush any pending events remaining in channel before exiting
    while let Ok(event) = rx.try_recv() {
        consumer.process_event(event);
    }
    do_flush(&session_stats, &db);
}

fn do_flush(session_stats: &SessionStats, db: &Mutex<StatsDatabase>) {
    let snapshots = session_stats.get_snapshots();
    if let Ok(mut db_guard) = db.lock() {
        for snapshot in &snapshots {
            if !snapshot.date.is_empty() {
                if let Ok(val) = serde_json::to_value(snapshot) {
                    db_guard.daily_buckets.insert(snapshot.date.clone(), val);
                }
            }
        }
        if snapshots.len() > 1 {
            session_stats.clear_rollover();
        }
    }
}

#[cfg(test)]
#[path = "../tests/Stats/StatsServiceTests.rs"]
mod tests;
