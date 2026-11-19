use super::super::storage::StatsDatabase;
use super::IStatsMigration;

pub(crate) struct RemoveDefaultDateBucketMigration;

pub(crate) fn is_default_date_bucket(date_key: &str) -> bool {
    date_key == "0001-01-01"
}

impl IStatsMigration for RemoveDefaultDateBucketMigration {
    fn version(&self) -> i32 {
        2
    }

    fn up(&self, db: &mut StatsDatabase) -> Result<(), String> {
        if is_default_date_bucket("0001-01-01") {
            db.daily_buckets.remove("0001-01-01");
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/Stats/Migrations/RemoveDefaultDateBucketMigrationTests.rs"]
mod tests;
