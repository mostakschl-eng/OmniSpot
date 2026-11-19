use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[path = "AppRegistryDocument.rs"]
pub(crate) mod app_registry_document;
#[path = "DailyBucketDocument.rs"]
pub(crate) mod daily_bucket_document;

#[allow(unused_imports)]
pub(crate) use app_registry_document::AppRegistryDocument;
#[allow(unused_imports)]
pub(crate) use daily_bucket_document::{AppUsageStats, DailyBucketDocument, FastestSwitchRecord};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MigrationRecord {
    pub version: i32,
    pub executed_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct StatsDatabase {
    pub migrations: Vec<MigrationRecord>,
    pub daily_buckets: HashMap<String, serde_json::Value>,
    pub app_registry: HashMap<String, String>,
}
