use std::sync::LazyLock;
use crate::fragmenter::stages::extract;

static RE_SUB_LEVELS: LazyLock<[regex::Regex; 6]> = LazyLock::new(|| [
    regex::Regex::new(r"(?m)^#{1,6}\s").unwrap(),
    regex::Regex::new(r"(?m)^#{2,6}\s").unwrap(),
    regex::Regex::new(r"(?m)^#{3,6}\s").unwrap(),
    regex::Regex::new(r"(?m)^#{4,6}\s").unwrap(),
    regex::Regex::new(r"(?m)^#{5,6}\s").unwrap(),
    regex::Regex::new(r"(?m)^#{6,6}\s").unwrap(),
]);

pub fn split_fragments_by_sub_headings(fragments: Vec<String>, heading_level: usize) -> Vec<String> {
    let sub_level = heading_level + 1;
    if sub_level > 6 {
        return fragments;
    }
    let idx = sub_level.saturating_sub(1).min(5);
    let sub_regex = &RE_SUB_LEVELS[idx];

    fragments.into_iter().flat_map(|fragment| {
        if !sub_regex.is_match(&fragment) {
            vec![fragment]
        } else {
            extract::split_by_heading_pattern(&fragment, sub_regex)
                .into_iter()
                .map(|s| s.to_string())
                .collect()
        }
    }).collect()
}
