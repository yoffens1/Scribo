use std::sync::LazyLock;
use regex::Regex;
use std::borrow::Cow;

static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^(?:\*\*\*+|---+|___+)\s*$").unwrap());

pub fn remove_horizontal_rules(text: &str) -> Cow<'_, str> {
    RE.replace_all(text, "")
}
