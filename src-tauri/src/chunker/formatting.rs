use std::borrow::Cow;
use std::sync::LazyLock;
use regex::Regex;

static RE_EMPTY_LINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n").unwrap());
pub fn remove_empty_lines(text: &str) -> Cow<'_, str> {
    RE_EMPTY_LINES.replace_all(text, "\n")
}

static RE_HR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^(?:\*\*\*+|---+|___+)\s*$").unwrap());
pub fn remove_horizontal_rules(text: &str) -> Cow<'_, str> {
    RE_HR.replace_all(text, "")
}

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

static RE_LIST_MARKERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^[\s]*[-+*]\s+").unwrap());
pub fn remove_list_markers(text: &str) -> Cow<'_, str> {
    RE_LIST_MARKERS.replace_all(text, "")
}

static RE_LIST_NUMBERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^[\s]*\d+\.\s+").unwrap());
pub fn remove_list_numbering(text: &str) -> Cow<'_, str> {
    RE_LIST_NUMBERS.replace_all(text, "")
}

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

static RE_HEADING_MARKERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s+").unwrap());
pub fn strip_heading_markers(text: &str) -> Cow<'_, str> {
    RE_HEADING_MARKERS.replace_all(text, "")
}
