use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicChunk {
    pub text: String,
    pub suggested_title: String,
}

pub struct RawBlock {
    pub range: std::ops::Range<usize>,
    pub text: String,
    pub is_heading_h1_h2: bool,
    pub heading_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateNote {
    pub note_id: i64,
    pub title: String,
    pub similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRecommendation {
    pub action: String, // "append" | "create_child" | "skip"
    pub target_note_id: Option<i64>,
    pub new_note_title: Option<String>,
    pub parent_note_id: Option<i64>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkDistributionPlan {
    pub chunk_index: usize,
    pub text: String,
    pub suggested_title: String,
    pub candidates: Vec<CandidateNote>,
    pub recommendation: LlmRecommendation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftDistributionPlan {
    pub draft_id: i64,
    pub chunks: Vec<ChunkDistributionPlan>,
}

pub fn extract_json_payload(raw: &str) -> Option<String> {
    let start_idx = raw.find('{')?;
    let end_idx = raw.rfind('}')?;
    if end_idx > start_idx {
        Some(raw[start_idx..=end_idx].to_string())
    } else {
        None
    }
}
