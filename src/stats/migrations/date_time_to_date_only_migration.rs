use super::super::storage::StatsDatabase;
use super::IStatsMigration;

pub(crate) struct DateTimeToDateOnlyMigration;

impl IStatsMigration for DateTimeToDateOnlyMigration {
    fn version(&self) -> i32 {
        1
    }

    fn up(&self, db: &mut StatsDatabase) -> Result<(), String> {
        let keys_to_fix: Vec<String> = db
            .daily_buckets
            .keys()
            .filter(|k| k.contains('T') || k.contains(':') || k.contains(' '))
            .cloned()
            .collect();

        for key in keys_to_fix {
            if let Some(val) = db.daily_buckets.remove(&key) {
                let date_part = key
                    .split(['T', ' '])
                    .next()
                    .unwrap_or(&key)
                    .to_string();
                db.daily_buckets.insert(date_part, val);
            }
        }

        Ok(())
    }
}
