#[path = "MigrationRecord.rs"]
pub mod migration_record;
#[path = "SettingsDocument.rs"]
pub mod settings_document;

pub use migration_record::MigrationRecord;
pub use settings_document::{ApplicationConfigurationDocument, SettingsDocument};
