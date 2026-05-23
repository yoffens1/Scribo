use std::sync::LazyLock;
use regex::Regex;
use std::borrow::Cow;

static RE_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\$[\s\S]*?\$\$").unwrap());
static RE_INLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$[^$]+\$").unwrap());

pub fn remove_latex(text: &str) -> Cow<'_, str> {
    let mut cleaned = RE_BLOCK.replace_all(text, "");
    if let Cow::Owned(s) = RE_INLINE.replace_all(&cleaned, "") { cleaned = Cow::Owned(s); }
    cleaned
}
