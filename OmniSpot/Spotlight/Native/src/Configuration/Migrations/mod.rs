#[path = "FixPackagedAppsMigration.rs"]
pub(crate) mod fix_packaged_apps_migration;
#[path = "IMigration.rs"]
pub(crate) mod i_migration;
#[path = "MigrationRunner.rs"]
pub(crate) mod migration_runner;

#[allow(unused_imports)]
pub(crate) use fix_packaged_apps_migration::FixPackagedAppsMigration;
#[allow(unused_imports)]
pub(crate) use i_migration::{ConfigurationDatabase, IMigration};
#[allow(unused_imports)]
pub(crate) use migration_runner::MigrationRunner;
