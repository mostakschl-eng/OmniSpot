use std::collections::HashSet;
use crate::stats::storage::MigrationRecord;
use super::i_migration::{ConfigurationDatabase, IMigration};

pub(crate) struct MigrationRunner<'a> {
    migrations: Vec<&'a dyn IMigration>,
}

impl<'a> MigrationRunner<'a> {
    pub(crate) fn new(migrations: Vec<&'a dyn IMigration>) -> Self {
        Self { migrations }
    }

    pub(crate) fn run_pending(&self, db: &mut ConfigurationDatabase) -> bool {
        let executed_versions: HashSet<i32> = db.migrations.iter().map(|m| m.version).collect();

        let mut pending: Vec<&&dyn IMigration> = self
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
#[path = "../../tests/Configuration/Migrations/MigrationRunnerTests.rs"]
mod tests;
