use std::sync::LazyLock;
use regex::Regex;
use std::borrow::Cow;

static RE_WIKI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").unwrap());
static RE_MD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\([^\)]+\)").unwrap());

pub fn remove_links(text: &str) -> Cow<'_, str> {
    let mut cleaned = RE_WIKI.replace_all(text, |caps: &regex::Captures| {
        if let Some(alias) = caps.get(2) {
            alias.as_str().to_string()
        } else {
            caps.get(1).unwrap().as_str().to_string()
        }
    });

    if let Cow::Owned(s) = RE_MD.replace_all(&cleaned, "$1") {
        cleaned = Cow::Owned(s);
    }
    cleaned
}
