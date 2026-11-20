pub(crate) mod date_time_to_date_only_migration;
pub(crate) mod i_stats_migration;
pub(crate) mod remove_default_date_bucket_migration;
pub(crate) mod stats_migration_runner;

#[allow(unused_imports)]
pub(crate) use date_time_to_date_only_migration::DateTimeToDateOnlyMigration;
#[allow(unused_imports)]
pub(crate) use i_stats_migration::IStatsMigration;
#[allow(unused_imports)]
pub(crate) use remove_default_date_bucket_migration::RemoveDefaultDateBucketMigration;
#[allow(unused_imports)]
pub(crate) use stats_migration_runner::StatsMigrationRunner;

static MIGRATION_1: RemoveDefaultDateBucketMigration = RemoveDefaultDateBucketMigration;
static MIGRATION_2: DateTimeToDateOnlyMigration = DateTimeToDateOnlyMigration;

pub(crate) fn all_migrations() -> Vec<&'static dyn IStatsMigration> {
    vec![&MIGRATION_1, &MIGRATION_2]
}
