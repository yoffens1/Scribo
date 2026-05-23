use std::sync::LazyLock;
use regex::Regex;
use std::borrow::Cow;

static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^[\s]*\d+\.\s+").unwrap());

pub fn remove_list_numbering(text: &str) -> Cow<'_, str> {
    RE.replace_all(text, "")
}
