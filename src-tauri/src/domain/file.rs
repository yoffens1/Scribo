use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileRef {
    pub file_id: i64,
    pub file_hash: Option<String>,
    pub is_deleted: Option<i64>,
    pub embedding_model: Option<String>,
    pub chunking_version: Option<String>,
    pub file_mtime: Option<i64>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpsertIndexingParams {
    pub clean_path: String,
    pub file_name: String,
    pub file_hash: String,
    pub file_mtime: Option<i64>,
    pub embedding_model: String,
    pub embedding_dim: i64,
    pub chunking_version: String,
    pub updated_at: i64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InsertFailedParams {
    pub clean_path: String,
    pub file_name: String,
    pub file_hash: String,
    pub file_mtime: Option<i64>,
    pub error: String,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileQueryRecord {
    pub file_id: i64,
    pub file_path: String,
    pub is_deleted: i64,
    pub file_mtime: Option<i64>,
    pub embedding_model: Option<String>,
    pub chunking_version: Option<String>,
}
