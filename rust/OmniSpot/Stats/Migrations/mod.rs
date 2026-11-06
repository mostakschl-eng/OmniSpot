use super::{session::DailyBucket, storage::StatsDocument};

pub(super) const CURRENT_VERSION: u32 = 2;

/// Applies the C# stats migration that removed the uninitialized default-date bucket.
pub(crate) fn remove_default_date_bucket(buckets: &mut Vec<DailyBucket>) {
    buckets.retain(|bucket| bucket.date != "0001-01-01");
}

pub(super) fn run_pending(document: &mut StatsDocument) -> Result<bool, String> {
    if document.schema_version > CURRENT_VERSION {
        return Err(format!(
            "Stats data schema {} is newer than this app supports ({CURRENT_VERSION}).",
            document.schema_version
        ));
    }
    if document.schema_version == CURRENT_VERSION {
        return Ok(false);
    }
    remove_default_date_bucket(&mut document.buckets);
    document.schema_version = CURRENT_VERSION;
    Ok(true)
}
