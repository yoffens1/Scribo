use std::sync::LazyLock;
use regex::Regex;
use std::borrow::Cow;

static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n").unwrap());

pub fn remove_empty_lines(text: &str) -> Cow<'_, str> {
    RE.replace_all(text, "\n")
}
