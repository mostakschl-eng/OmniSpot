use std::collections::HashSet;
use super::super::storage::{MigrationRecord, StatsDatabase};
use super::IStatsMigration;

pub(crate) struct StatsMigrationRunner<'a> {
    db_exists: bool,
    migrations: Vec<&'a dyn IStatsMigration>,
}

impl<'a> StatsMigrationRunner<'a> {
    pub(crate) fn new(db_exists: bool, migrations: Vec<&'a dyn IStatsMigration>) -> Self {
        Self {
            db_exists,
            migrations,
        }
    }

    pub(crate) fn run_pending(&self, db: &mut StatsDatabase) -> bool {
        if !self.db_exists {
            return true;
        }

        let executed_versions: HashSet<i32> = db.migrations.iter().map(|m| m.version).collect();

        let mut pending: Vec<&&dyn IStatsMigration> = self
            .migrations
            .iter()
            .filter(|m| !executed_versions.contains(&m.version()))
            .collect();

        pending.sort_by_key(|m| m.version());

        if pending.is_empty() {
            return true;
        }

        for migration in pending {
            let version = migration.version();
            let backup = db.clone();
            match migration.up(db) {
                Ok(()) => {
                    db.migrations.push(MigrationRecord {
                        version,
                        executed_at: "now".to_string(),
                    });
                }
                Err(_) => {
                    *db = backup;
                    return false;
                }
            }
        }

        true
    }
}

#[cfg(test)]
#[path = "../../tests/Stats/Migrations/StatsMigrationRunnerTests.rs"]
mod tests;
