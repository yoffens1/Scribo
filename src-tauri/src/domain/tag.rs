//! Tag — hierarchical metadata for notes and fragments.
//!
//! Tags are organized in a tree structure (`parent_tag_id`) and have a materialized path (`path_cached`)
//! for fast subtree queries. They can be applied manually by the user or automatically by AI taxonomy.

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

/// A hierarchical tag in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    /// Unique identifier for the tag.
    pub tag_id: TagId,
    /// Reference to the parent tag, if this is a sub-tag.
    pub parent_tag_id: Option<TagId>,
    /// Display name of the tag (e.g. "Data Science").
    pub name: String,
    /// URL-friendly identifier slug of the tag (e.g. "data-science").
    pub slug: String,
    /// Optional color code (hex) for UI rendering.
    pub color: Option<String>,
    /// Optional icon name/emoji for UI rendering.
    pub icon: Option<String>,
    /// Depth level in the hierarchy tree (0 for root tags).
    pub depth: i64,
    /// Materialized hierarchical path of slugs (e.g. "math/calculus").
    pub path_cached: String,
    /// Optional description text for documentation.
    pub description: Option<String>,
    /// Creation timestamp in UTC seconds.
    pub created_at: Timestamp,
    /// Last update timestamp in UTC seconds.
    pub updated_at: Timestamp,
}

impl Tag {
    /// Computes the cached path for a tag based on its parent's path and its own slug.
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

    /// Computes the depth of a tag based on its parent's depth.
    pub fn compute_depth(parent_depth: Option<i64>) -> i64 {
        parent_depth.map(|d| d + 1).unwrap_or(0)
    }
}

/// Payload for creating a new tag.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewTag {
    /// Reference to the parent tag, if this is a sub-tag.
    pub parent_tag_id: Option<TagId>,
    /// Display name of the new tag.
    pub name: String,
    /// Slug of the new tag.
    pub slug: String,
    /// Optional hex color code.
    pub color: Option<String>,
    /// Optional icon string.
    pub icon: Option<String>,
    /// Optional description text.
    pub description: Option<String>,
}

/// Identifies how a tag was applied to an entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TagSource {
    /// Applied manually by the user.
    Manual,
    /// Suggested/applied automatically by an AI model.
    Ai,
    /// Inherited automatically from a parent resource.
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

/// A many-to-many relationship linking a Note to a Tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteTagRelation {
    /// Note identifier.
    pub note_id: crate::domain::NoteId,
    /// Tag identifier.
    pub tag_id: TagId,
    /// Origin of this tag relationship.
    pub source: TagSource,
    /// Optional confidence score if the tag was assigned by an AI model.
    pub confidence: Option<f64>,
    /// Time when the relation was created.
    pub created_at: Timestamp,
}

/// A many-to-many relationship linking a Fragment to a Tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentTagRelation {
    /// Fragment identifier.
    pub fragment_id: crate::domain::fragment::FragmentId,
    /// Tag identifier.
    pub tag_id: TagId,
    /// Origin of this tag relationship.
    pub source: TagSource,
    /// Time when the relation was created.
    pub created_at: Timestamp,
}
