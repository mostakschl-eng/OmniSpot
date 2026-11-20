use super::super::storage::SettingsDocument;
use crate::stats::storage::MigrationRecord;

#[derive(Clone, Debug, Default)]
pub(crate) struct ConfigurationDatabase {
    pub settings: Option<SettingsDocument>,
    pub migrations: Vec<MigrationRecord>,
}

pub(crate) trait IMigration {
    fn version(&self) -> i32;
    fn up(&self, db: &mut ConfigurationDatabase) -> Result<(), String>;
}
