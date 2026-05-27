use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicChunk {
    pub text: String,
    pub suggested_title: String,
}

#[derive(Debug, Clone)]
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
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DistributeAction {
    Append { 
        target_note_id: crate::domain::NoteId,
        target_section_id: Option<crate::domain::SectionId>,
    },
    CreateChild { 
        parent_note_id: Option<crate::domain::NoteId>,
        new_note_title: String,
    },
    MergeWithChunk { 
        chunk_index: usize,
    },
    Skip { 
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRecommendation {
    #[serde(flatten)]
    pub action: DistributeAction,
    pub tags: Option<Vec<String>>,
    pub confidence: Option<f32>,
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
