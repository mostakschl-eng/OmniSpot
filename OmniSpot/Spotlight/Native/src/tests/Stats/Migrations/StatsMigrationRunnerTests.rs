use super::*;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

struct FakeMigration {
    version: i32,
    throws: bool,
    was_run: Arc<AtomicBool>,
    run_count: Arc<AtomicI32>,
    on_run: Option<Box<dyn Fn() + Send + Sync>>,
}

impl FakeMigration {
    fn new(version: i32) -> Self {
        Self {
            version,
            throws: false,
            was_run: Arc::new(AtomicBool::new(false)),
            run_count: Arc::new(AtomicI32::new(0)),
            on_run: None,
        }
    }

    fn failing(version: i32) -> Self {
        Self {
            version,
            throws: true,
            was_run: Arc::new(AtomicBool::new(false)),
            run_count: Arc::new(AtomicI32::new(0)),
            on_run: None,
        }
    }

    fn with_action<F: Fn() + Send + Sync + 'static>(version: i32, f: F) -> Self {
        Self {
            version,
            throws: false,
            was_run: Arc::new(AtomicBool::new(false)),
            run_count: Arc::new(AtomicI32::new(0)),
            on_run: Some(Box::new(f)),
        }
    }
}

impl IStatsMigration for FakeMigration {
    fn version(&self) -> i32 {
        self.version
    }

    fn up(&self, _db: &mut StatsDatabase) -> Result<(), String> {
        self.was_run.store(true, Ordering::SeqCst);
        self.run_count.fetch_add(1, Ordering::SeqCst);
        if let Some(ref action) = self.on_run {
            action();
        }
        if self.throws {
            Err("migration error".to_string())
        } else {
            Ok(())
        }
    }
}

#[test]
fn run_pending_returns_true_when_no_migrations_registered() {
    let mut db = StatsDatabase::default();
    let runner = StatsMigrationRunner::new(true, vec![]);
    assert!(runner.run_pending(&mut db));
}

#[test]
fn run_pending_runs_migration_when_not_yet_executed() {
    let mut db = StatsDatabase::default();
    let m = FakeMigration::new(1);
    let runner = StatsMigrationRunner::new(true, vec![&m]);

    assert!(runner.run_pending(&mut db));
    assert!(m.was_run.load(Ordering::SeqCst));
    assert_eq!(db.migrations.len(), 1);
    assert_eq!(db.migrations[0].version, 1);
}

#[test]
fn run_pending_records_migration_version_and_skips_on_repeat() {
    let mut db = StatsDatabase::default();
    let m = FakeMigration::new(1);
    let runner = StatsMigrationRunner::new(true, vec![&m]);

    assert!(runner.run_pending(&mut db));
    assert!(runner.run_pending(&mut db));
    assert_eq!(m.run_count.load(Ordering::SeqCst), 1);
}

#[test]
fn run_pending_does_not_record_version_when_migration_fails() {
    let mut db = StatsDatabase::default();
    let failing = FakeMigration::failing(1);
    let runner = StatsMigrationRunner::new(true, vec![&failing]);

    assert!(!runner.run_pending(&mut db));
    assert!(db.migrations.is_empty());

    let retry = FakeMigration::new(1);
    let runner2 = StatsMigrationRunner::new(true, vec![&retry]);
    assert!(runner2.run_pending(&mut db));
    assert!(retry.was_run.load(Ordering::SeqCst));
}

#[test]
fn run_pending_skips_migration_when_already_recorded() {
    let mut db = StatsDatabase::default();
    db.migrations.push(MigrationRecord {
        version: 1,
        executed_at: "now".to_string(),
    });

    let m = FakeMigration::new(1);
    let runner = StatsMigrationRunner::new(true, vec![&m]);
    assert!(runner.run_pending(&mut db));
    assert!(!m.was_run.load(Ordering::SeqCst));
}

#[test]
fn run_pending_runs_migrations_in_version_order() {
    let mut db = StatsDatabase::default();
    let order = Arc::new(std::sync::Mutex::new(Vec::new()));

    let o1 = order.clone();
    let m3 = FakeMigration::with_action(3, move || o1.lock().unwrap().push(3));
    let o2 = order.clone();
    let m1 = FakeMigration::with_action(1, move || o2.lock().unwrap().push(1));
    let o3 = order.clone();
    let m2 = FakeMigration::with_action(2, move || o3.lock().unwrap().push(2));

    let runner = StatsMigrationRunner::new(true, vec![&m3, &m1, &m2]);
    assert!(runner.run_pending(&mut db));

    let executed = order.lock().unwrap().clone();
    assert_eq!(executed, vec![1, 2, 3]);
}

#[test]
fn run_pending_stops_after_failure() {
    let mut db = StatsDatabase::default();
    let m1 = FakeMigration::failing(1);
    let m2 = FakeMigration::new(2);

    let runner = StatsMigrationRunner::new(true, vec![&m1, &m2]);
    assert!(!runner.run_pending(&mut db));
    assert!(!m2.was_run.load(Ordering::SeqCst));
}

#[test]
fn run_pending_returns_true_when_db_does_not_exist() {
    let mut db = StatsDatabase::default();
    let m = FakeMigration::new(1);
    let runner = StatsMigrationRunner::new(false, vec![&m]);

    assert!(runner.run_pending(&mut db));
    assert!(!m.was_run.load(Ordering::SeqCst));
}
