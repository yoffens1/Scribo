use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::ai::LlmConfig;
use crate::domain::note::NoteId;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineConfig {
    pub auto_translate: Option<bool>,
    pub expand_synonyms: Option<String>, // "off", "static", "llm"
    pub synonym_dict: Option<HashMap<String, Vec<String>>>,
    pub hyde: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRerankConfig {
    pub enabled: bool,
    pub mode: Option<String>, // "scoring", "listwise"
    pub max_candidates: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalConfig {
    pub mode: String, // "embedding", "keyword", "hybrid"
    pub embedding_weight: Option<f32>,
    pub pipeline: Option<PipelineConfig>,
    pub ai_rerank: Option<AiRerankConfig>,
    pub vault_lang: Option<String>,
    pub llm_config: Option<LlmConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmentRef {
    pub note_id: NoteId,
    pub fragment_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub fragment_ref: FragmentRef,
    pub score: f32,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrieveFilters {
    pub note_id: Option<NoteId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrieveOptions {
    pub top_k: Option<usize>,
    pub filters: Option<RetrieveFilters>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchQuery {
    pub note_id: Option<NoteId>,
    pub include_deleted: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResult {
    pub fragment_id: Option<i64>,
    pub note_id: NoteId,
    pub fragment_index: usize,
    pub fragment_text: Option<String>,
    pub token_count: Option<i64>,
    #[serde(with = "serde_bytes")]
    pub embedding: Vec<u8>,
}
