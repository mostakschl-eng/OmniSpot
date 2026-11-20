use super::super::storage::StatsDatabase;

pub(crate) trait IStatsMigration {
    fn version(&self) -> i32;
    fn up(&self, db: &mut StatsDatabase) -> Result<(), String>;
}
