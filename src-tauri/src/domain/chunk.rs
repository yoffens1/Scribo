use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub chunk_id: i64,
    pub file_path: String,
    pub chunk_index: i64,
    pub chunk_text: Option<String>,
    pub token_count: Option<i64>,
    #[serde(with = "serde_bytes")]
    pub embedding: Vec<u8>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChunkInsertRow {
    pub chunk_index: i64,
    pub text: String,
    pub tokens: i64,
    #[serde(with = "serde_bytes")]
    pub embedding: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub chunk_id: i64,
    pub file_path: String,
    pub chunk_index: i64,
    pub snippet: String,
    pub score: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VectorSearchHit {
    pub chunk_id: i64,
    pub file_path: String,
    pub chunk_index: i64,
    pub chunk_text: Option<String>,
    pub similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkKind {
    pub name: String,
}
