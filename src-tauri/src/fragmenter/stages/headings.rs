use crate::fragmenter::types::FragmentOptions;
use super::clean::clean_fragment;

pub fn prepend_heading_to_fragments(fragments: Vec<String>,
    section_heading: &str,
    options: &FragmentOptions,
) -> Vec<String> {
    let clean_heading = clean_fragment(section_heading, options).trim().to_string();
    if clean_heading.is_empty() {
        return fragments;
    }

    fragments
        .into_iter()
        .filter(|fragment| fragment != &clean_heading)
        .map(|fragment| {
            let first_line = fragment.trim_start().lines().next().unwrap_or("").trim();
            if first_line == clean_heading {
                fragment
            } else {
                format!("{}\n{}", clean_heading, fragment)
            }
        })
        .collect()
}
