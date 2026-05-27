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
    Skip,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{NoteId, SectionId};

    #[test]
    fn test_deserialize_append_action() {
        let json = r#"{
            "action": "append",
            "target_note_id": 42,
            "target_section_id": 100,
            "tags": ["math", "calculus"],
            "confidence": 0.95,
            "reason": "Belongs to calculus section"
        }"#;

        let rec: LlmRecommendation = serde_json::from_str(json).unwrap();
        assert!(matches!(rec.action, DistributeAction::Append { 
            target_note_id: NoteId(42), 
            target_section_id: Some(SectionId(100)) 
        }));
        assert_eq!(rec.tags.unwrap(), vec!["math".to_string(), "calculus".to_string()]);
        assert_eq!(rec.confidence, Some(0.95));
        assert_eq!(rec.reason, "Belongs to calculus section");
    }

    #[test]
    fn test_deserialize_create_child_action() {
        let json = r#"{
            "action": "create_child",
            "parent_note_id": null,
            "new_note_title": "Limits",
            "tags": [],
            "confidence": 0.8,
            "reason": "New subtopic"
        }"#;

        let rec: LlmRecommendation = serde_json::from_str(json).unwrap();
        assert!(matches!(rec.action, DistributeAction::CreateChild { 
            parent_note_id: None, 
            new_note_title 
        } if new_note_title == "Limits"));
    }

    #[test]
    fn test_deserialize_skip_action() {
        let json = r#"{
            "action": "skip",
            "reason": "Not relevant"
        }"#;

        let rec: LlmRecommendation = serde_json::from_str(json).unwrap();
        assert!(matches!(rec.action, DistributeAction::Skip));
        assert_eq!(rec.reason, "Not relevant");
    }
}


