use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomChunk {
    pub hash: String,
    pub embedding_text: String,
    pub generation_text: String,
    pub index: usize,
    pub source_path: String,
    #[serde(default)]
    pub is_table: bool,
    pub question_heading: Option<String>,
    pub filename: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum ChunkDecision {
    Keep {
        chunk: AtomChunk,
        reason: String,
    },
    Merge {
        target_path: String,
        source_chunk: AtomChunk,
        reason: String,
    },
    Reject {
        chunk: AtomChunk,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeduplicationResult {
    pub decisions: Vec<ChunkDecision>,
    pub remaining: Vec<AtomChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedTaxonomy {
    pub roots: Vec<FolderNode>,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderNode {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub children: Vec<FolderNode>,
    #[serde(default)]
    pub assigned_chunks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementPlan {
    #[serde(default)]
    pub decisions: Vec<PlacementDecision>,
    #[serde(default)]
    pub folders_to_create: Vec<String>,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementDecision {
    pub chunk_hash: String,
    pub output_path: String,
    pub action: String, // "create" | "merge" | "rename" | "nest"
    pub reason: String,
    pub existing_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WriteOperation {
    CreateFile { path: String, content: String },
    MergeChunk { source_file: String, target_file: String, chunk_text: String },
    CreateFolder { path: String },
    MoveFile { from: String, to: String },
    DeleteFile { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefineryResult {
    pub source_path: String,
    pub chunks: Vec<AtomChunk>,
    pub dedup: DeduplicationResult,
    pub taxonomy: ProposedTaxonomy,
    pub placement: PlacementPlan,
    pub operations: Vec<WriteOperation>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRefineryResult {
    pub results: Vec<RefineryResult>,
    pub errors: Vec<RefineryError>,
    pub total_chunks: usize,
    pub merged_chunks: usize,
    pub created_files: usize,
    pub created_folders: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefineryError {
    pub source_path: String,
    pub error: String,
}
