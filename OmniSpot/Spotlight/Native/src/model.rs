use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Candidate {
    pub id: u32,
    pub title: String,
    #[serde(default)]
    pub alternate_name: String,
    #[serde(default)]
    pub base_score: i32,
    #[serde(default)]
    pub usage_boost: i32,
    #[serde(default)]
    pub normalized_title: String,
    #[serde(default)]
    pub normalized_alternate_name: String,
    #[serde(default)]
    pub character_mask: [u64; 2],
}

#[derive(Debug, Serialize)]
pub(crate) struct Match {
    pub id: u32,
    pub score: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IndexedResult {
    pub title: String,
    pub subtitle: String,
    pub target: String,
    pub result_type: String,
    pub score: i32,
    pub icon_path: String,
}
