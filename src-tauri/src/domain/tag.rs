use serde::{Deserialize, Serialize};
use super::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TagId(pub i64);

impl From<i64> for TagId {
    fn from(v: i64) -> Self {
        Self(v)
    }
}

impl std::fmt::Display for TagId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub tag_id: TagId,
    pub parent_tag_id: Option<TagId>,
    pub name: String,
    pub slug: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub depth: i64,
    pub path_cached: String,
    pub description: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Tag {
    pub fn compute_path(parent_path: Option<&str>, slug: &str) -> String {
        match parent_path {
            None => slug.to_string(),
            Some(pp) => {
                if pp.is_empty() {
                    slug.to_string()
                } else {
                    format!("{}/{}", pp, slug)
                }
            }
        }
    }

    pub fn compute_depth(parent_depth: Option<i64>) -> i64 {
        parent_depth.map(|d| d + 1).unwrap_or(0)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewTag {
    pub parent_tag_id: Option<TagId>,
    pub name: String,
    pub slug: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TagSource {
    Manual,
    Ai,
    Inherited,
}

impl std::fmt::Display for TagSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TagSource::Manual => "manual",
            TagSource::Ai => "ai",
            TagSource::Inherited => "inherited",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for TagSource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(TagSource::Manual),
            "ai" => Ok(TagSource::Ai),
            "inherited" => Ok(TagSource::Inherited),
            _ => Err(format!("Unknown tag source: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteTagRelation {
    pub note_id: crate::domain::NoteId,
    pub tag_id: TagId,
    pub source: TagSource,
    pub confidence: Option<f64>,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentTagRelation {
    pub fragment_id: crate::domain::fragment::FragmentId,
    pub tag_id: TagId,
    pub source: TagSource,
    pub created_at: Timestamp,
}
