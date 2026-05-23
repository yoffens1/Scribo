use std::sync::LazyLock;
use regex::Regex;
use std::borrow::Cow;

static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s+").unwrap());

pub fn strip_heading_markers(text: &str) -> Cow<'_, str> {
    RE.replace_all(text, "")
}
