use std::ops::Range;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fragment {
    pub text: String,
    pub meta: FragmentMeta,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmentMeta {
    pub index: usize,
    pub source_range: Option<Range<usize>>,
    pub heading_path: Vec<String>,
    pub suggested_title: Option<String>,
    pub is_top_level_section: bool,
    pub token_count: usize,
    pub char_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmenterResult {
    pub fragments: Vec<Fragment>,
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmenterPair {
    pub embedding: String,
    pub generation: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmenterPairedResult {
    pub pairs: Vec<FragmenterPair>,
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}
