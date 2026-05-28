//! Domain types for the "Distribute Draft" AI pipeline.
//!
//! Describes the data structures used when breaking a large draft into smaller chunks
//! and recommending where those chunks should be moved within the existing note hierarchy.

use serde::{Deserialize, Serialize};

/// A cohesive block of text extracted from a draft, along with an AI-generated summary/title.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicChunk {
    /// The actual text content of the chunk.
    pub text: String,
    /// AI-generated heading/title for this block of text.
    pub suggested_title: String,
}

/// Intermediate structure used during AST parsing. Represents a chunk of markdown
/// bounded by its structural headings.
#[derive(Debug, Clone)]
pub struct RawBlock {
    /// Byte range of this block in the raw source content.
    pub range: std::ops::Range<usize>,
    /// Raw text content of the block.
    pub text: String,
    /// True if this block starts with an H1 or H2 header.
    pub is_heading_h1_h2: bool,
    /// Heading text if this block is a heading.
    pub heading_title: Option<String>,
}

/// A prospective parent note for a chunk, retrieved via semantic search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateNote {
    /// Identifier of the candidate note.
    pub note_id: i64,
    /// Title of the candidate note.
    pub title: String,
    /// Semantic similarity score (higher is more similar).
    pub similarity: f32,
}

/// The action recommended by the LLM for a specific chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DistributeAction {
    /// Append the chunk text to an existing note (optionally under a specific section).
    Append { 
        /// Identifier of the target note.
        target_note_id: crate::domain::NoteId,
        /// Optional identifier of the section to append under.
        target_section_id: Option<crate::domain::SectionId>,
    },
    /// Create a new child note under a parent note.
    CreateChild { 
        /// Parent note identifier, if creating under a parent.
        parent_note_id: Option<crate::domain::NoteId>,
        /// Title for the new child note.
        new_note_title: String,
    },
    /// Merge this chunk with another chunk in the same distribution session.
    MergeWithChunk { 
        /// 0-based index of the target chunk to merge with.
        chunk_index: usize,
    },
    /// Discard this chunk (e.g. if it contains irrelevant information).
    Skip,
}

/// The full recommendation object returned by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRecommendation {
    /// The concrete action recommended by the LLM.
    #[serde(flatten)]
    pub action: DistributeAction,
    /// Optional tags to apply to the target/new note.
    pub tags: Option<Vec<String>>,
    /// Confidence score of the recommendation.
    pub confidence: Option<f32>,
    /// Explanatory reasoning for the recommended action.
    pub reason: String,
}

/// The final distribution plan for a single chunk, presented to the user for approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkDistributionPlan {
    /// 0-based index of the chunk.
    pub chunk_index: usize,
    /// Text content of this chunk.
    pub text: String,
    /// Suggested heading or title.
    pub suggested_title: String,
    /// Top candidate notes retrieved via vector search.
    pub candidates: Vec<CandidateNote>,
    /// Recommendation computed by the LLM.
    pub recommendation: LlmRecommendation,
}

/// The complete distribution plan for an entire draft.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftDistributionPlan {
    /// Identifier of the draft note.
    pub draft_id: i64,
    /// List of chunk-specific plans.
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


