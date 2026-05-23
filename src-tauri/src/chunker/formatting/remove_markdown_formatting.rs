use std::sync::LazyLock;
use regex::Regex;
use std::borrow::Cow;

static RE_BOLD1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*(.+?)\*\*").unwrap());
static RE_BOLD2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"__(.+?)__").unwrap());
static RE_STRIKE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"~~(.+?)~~").unwrap());
static RE_HI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"==(.+?)==").unwrap());
static RE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`(.+?)`").unwrap());
static RE_ITALIC_UNDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(^|\s)_([^_]+)_(\s|$)").unwrap());
static RE_ITALIC_STAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(^|\s)\*([^*]+)\*(\s|$)").unwrap());

pub fn remove_markdown_formatting(text: &str) -> Cow<'_, str> {
    let mut cleaned = RE_BOLD1.replace_all(text, "$1");
    if let Cow::Owned(s) = RE_BOLD2.replace_all(&cleaned, "$1") { cleaned = Cow::Owned(s); }
    if let Cow::Owned(s) = RE_STRIKE.replace_all(&cleaned, "$1") { cleaned = Cow::Owned(s); }
    if let Cow::Owned(s) = RE_HI.replace_all(&cleaned, "$1") { cleaned = Cow::Owned(s); }
    if let Cow::Owned(s) = RE_CODE.replace_all(&cleaned, "$1") { cleaned = Cow::Owned(s); }
    if let Cow::Owned(s) = RE_ITALIC_UNDER.replace_all(&cleaned, "${1}${2}${3}") { cleaned = Cow::Owned(s); }
    if let Cow::Owned(s) = RE_ITALIC_STAR.replace_all(&cleaned, "${1}${2}${3}") { cleaned = Cow::Owned(s); }
    cleaned
}
