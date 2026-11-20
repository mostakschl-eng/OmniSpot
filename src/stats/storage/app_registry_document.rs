use serde::{Deserialize, Serialize};

pub const COLLECTION_NAME: &str = "app_registry";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRegistryDocument {
    #[serde(rename = "_id", default)]
    pub process_name: String,
    #[serde(default)]
    pub display_name: String,
}

impl AppRegistryDocument {
    pub fn new(process_name: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            process_name: process_name.into(),
            display_name: display_name.into(),
        }
    }
}
