use std::sync::LazyLock;
use regex::Regex;

static RE_HEADING_EXACT: LazyLock<[Regex; 6]> = LazyLock::new(|| {
    [
        Regex::new(r"(?m)^#\s").unwrap(),
        Regex::new(r"(?m)^##\s").unwrap(),
        Regex::new(r"(?m)^###\s").unwrap(),
        Regex::new(r"(?m)^####\s").unwrap(),
        Regex::new(r"(?m)^#####\s").unwrap(),
        Regex::new(r"(?m)^######\s").unwrap(),
    ]
});
static RE_ANY_HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s").unwrap());

pub fn split_by_heading_pattern<'a>(text: &'a str, heading_regex: &Regex) -> Vec<&'a str> {
    let mut sections = Vec::new();
    let mut last_idx = 0;

    for mat in heading_regex.find_iter(text) {
        if mat.start() > last_idx {
            let section = &text[last_idx..mat.start()];
            let mut trimmed = section;
            if trimmed.ends_with('\n') {
                trimmed = &trimmed[..trimmed.len() - 1];
            }
            if trimmed.ends_with('\r') {
                trimmed = &trimmed[..trimmed.len() - 1];
            }
            sections.push(trimmed);
        }
        last_idx = mat.start();
    }

    if last_idx < text.len() {
        sections.push(&text[last_idx..]);
    }

    sections
}

pub fn split_by_headings(text: &str, level: usize) -> Vec<&str> {
    let heading_regex = if (1..=6).contains(&level) {
        &RE_HEADING_EXACT[level - 1]
    } else {
        &RE_ANY_HEADING
    };

    split_by_heading_pattern(text, heading_regex)
}
