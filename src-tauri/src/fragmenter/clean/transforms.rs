use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;
use regex::Regex;
use crate::fragmenter::config::CleanFlags;

static RE_EMPTY_LINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n").unwrap());
static RE_HR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^(?:\*\*\*+|---+|___+)\s*$").unwrap());
static RE_WIKI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").unwrap());
static RE_MD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\([^\)]+\)").unwrap());
static RE_LIST_MARKERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^[\s]*[-+*]\s+").unwrap());
static RE_LIST_NUMBERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^[\s]*\d+\.\s+").unwrap());
static RE_BOLD1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*(.+?)\*\*").unwrap());
static RE_BOLD2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"__(.+?)__").unwrap());
static RE_STRIKE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"~~(.+?)~~").unwrap());
static RE_HI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"==(.+?)==").unwrap());
static RE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`(.+?)`").unwrap());
static RE_ITALIC_UNDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(^|\s)_([^_]+)_(\s|$)").unwrap());
static RE_ITALIC_STAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(^|\s)\*([^*]+)\*(\s|$)").unwrap());
static RE_HEADING_MARKERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s+").unwrap());

static SYMBOL_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        ("alpha", "α"), ("beta", "β"), ("gamma", "γ"),
        ("delta", "δ"), ("epsilon", "ε"), ("zeta", "ζ"),
        ("eta", "η"), ("theta", "θ"), ("iota", "ι"),
        ("kappa", "κ"), ("lambda", "λ"), ("mu", "μ"),
        ("nu", "ν"), ("xi", "ξ"), ("pi", "π"),
        ("rho", "ρ"), ("sigma", "σ"), ("tau", "τ"),
        ("upsilon", "υ"), ("phi", "φ"), ("chi", "χ"),
        ("psi", "ψ"), ("omega", "ω"),
        
        ("Gamma", "Γ"), ("Delta", "Δ"), ("Theta", "Θ"),
        ("Lambda", "Λ"), ("Xi", "Ξ"), ("Pi", "Π"),
        ("Sigma", "Σ"), ("Upsilon", "Υ"), ("Phi", "Φ"),
        ("Psi", "Ψ"), ("Omega", "Ω"),

        ("forall", "∀"), ("exists", "∃"),
        ("neg", "¬"), ("lnot", "¬"), ("land", "∧"), ("wedge", "∧"),
        ("lor", "∨"), ("vee", "∨"), ("rightarrow", "→"), ("to", "→"),
        ("leftrightarrow", "↔"), ("Rightarrow", "⇒"),
        ("Leftrightarrow", "⇔"), ("top", "⊤"),
        ("bot", "⊥"), ("vdash", "⊢"),
        ("models", "⊨"), ("equiv", "≡"),

        ("sum", "∑"), ("prod", "∏"),
        ("int", "∫"), ("infty", "∞"),

        ("cdot", "·"), ("times", "×"),
        ("div", "÷"), ("pm", "±"),
        ("mp", "∓"), ("ast", "*"),

        ("leq", "≤"), ("geq", "≥"),
        ("neq", "≠"), ("approx", "≈"),
        ("propto", "∝"), ("sim", "∼"),

        ("leftarrow", "←"),
    ])
});

static RE_LATEX_SYMBOLS: LazyLock<Regex> = LazyLock::new(|| {
    let keys: Vec<&str> = SYMBOL_MAP.keys().copied().collect();
    let pattern = format!(r"\\({})(?-u:\b)", keys.join("|"));
    Regex::new(&pattern).unwrap()
});

static RE_LATEX_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)\$\$([\s\S]*?)\$\$").unwrap());
static RE_LATEX_INLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)\$([^$]+)\$").unwrap());

static RE_SUM_SUB_SUP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\sum\s*_\{([^}]+)\}\s*\^\{([^}]+)\}").unwrap());
static RE_SUM_SUB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\sum\s*_\{([^}]+)\}").unwrap());
static RE_SUM_SUP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\sum\s*\^\{([^}]+)\}").unwrap());
static RE_PROD_SUB_SUP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\prod\s*_\{([^}]+)\}\s*\^\{([^}]+)\}").unwrap());
static RE_PROD_SUB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\prod\s*_\{([^}]+)\}").unwrap());
static RE_PROD_SUP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\prod\s*\^\{([^}]+)\}").unwrap());
static RE_VEC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\vec\{([^}]+)\}").unwrap());
static RE_HAT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\hat\{([^}]+)\}").unwrap());
static RE_TILDE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\tilde\{([^}]+)\}").unwrap());
static RE_BAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\bar\{([^}]+)\}").unwrap());
static RE_FRAC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\frac\{([^}]+)\}\{([^}]+)\}").unwrap());
static RE_BRACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[{}]").unwrap());
static RE_UNKNOWN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\[a-zA-Z]+").unwrap());

pub fn remove_empty_lines(text: &str) -> Cow<'_, str> {
    RE_EMPTY_LINES.replace_all(text, "\n")
}

pub fn remove_horizontal_rules(text: &str) -> Cow<'_, str> {
    RE_HR.replace_all(text, "")
}

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

pub fn remove_list_markers(text: &str) -> Cow<'_, str> {
    RE_LIST_MARKERS.replace_all(text, "")
}

pub fn remove_list_numbering(text: &str) -> Cow<'_, str> {
    RE_LIST_NUMBERS.replace_all(text, "")
}

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

pub fn strip_heading_markers(text: &str) -> Cow<'_, str> {
    RE_HEADING_MARKERS.replace_all(text, "")
}

pub fn format_latex(text: &str) -> Cow<'_, str> {
    let text1 = RE_LATEX_BLOCK.replace_all(text, |caps: &regex::Captures| {
        format!("$${}$$", transform_math(&caps[1]))
    });
    
    match RE_LATEX_INLINE.replace_all(&text1, |caps: &regex::Captures| {
        format!("${}$", transform_math(&caps[1]))
    }) {
        Cow::Owned(s) => Cow::Owned(s),
        Cow::Borrowed(_) => text1,
    }
}

fn transform_math(s: &str) -> String {
    let mut cleaned = Cow::Borrowed(s);

    if let Cow::Owned(new_s) = RE_LATEX_SYMBOLS.replace_all(&cleaned, |caps: &regex::Captures| {
        SYMBOL_MAP.get(&caps[1]).unwrap().to_string()
    }) {
        cleaned = Cow::Owned(new_s);
    }

    if let Cow::Owned(new_s) = RE_SUM_SUB_SUP.replace_all(&cleaned, "∑_{$1}^{$2}") { cleaned = Cow::Owned(new_s); }
    if let Cow::Owned(new_s) = RE_SUM_SUB.replace_all(&cleaned, "∑_{$1}") { cleaned = Cow::Owned(new_s); }
    if let Cow::Owned(new_s) = RE_SUM_SUP.replace_all(&cleaned, "∑^{$1}") { cleaned = Cow::Owned(new_s); }
    
    if let Cow::Owned(new_s) = RE_PROD_SUB_SUP.replace_all(&cleaned, "∏_{$1}^{$2}") { cleaned = Cow::Owned(new_s); }
    if let Cow::Owned(new_s) = RE_PROD_SUB.replace_all(&cleaned, "∏_{$1}") { cleaned = Cow::Owned(new_s); }
    if let Cow::Owned(new_s) = RE_PROD_SUP.replace_all(&cleaned, "∏^{$1}") { cleaned = Cow::Owned(new_s); }

    if let Cow::Owned(new_s) = RE_VEC.replace_all(&cleaned, "$1⃗") { cleaned = Cow::Owned(new_s); }
    if let Cow::Owned(new_s) = RE_HAT.replace_all(&cleaned, "$1̂") { cleaned = Cow::Owned(new_s); }
    if let Cow::Owned(new_s) = RE_TILDE.replace_all(&cleaned, "$1̃") { cleaned = Cow::Owned(new_s); }
    if let Cow::Owned(new_s) = RE_BAR.replace_all(&cleaned, "$1̅") { cleaned = Cow::Owned(new_s); }
    if let Cow::Owned(new_s) = RE_FRAC.replace_all(&cleaned, "($1)/($2)") { cleaned = Cow::Owned(new_s); }
    if let Cow::Owned(new_s) = RE_BRACES.replace_all(&cleaned, "") { cleaned = Cow::Owned(new_s); }
    if let Cow::Owned(new_s) = RE_UNKNOWN.replace_all(&cleaned, "") { cleaned = Cow::Owned(new_s); }

    cleaned.into_owned()
}

pub fn remove_latex(text: &str) -> Cow<'_, str> {
    let mut cleaned = RE_LATEX_BLOCK.replace_all(text, "");
    if let Cow::Owned(s) = RE_LATEX_INLINE.replace_all(&cleaned, "") { cleaned = Cow::Owned(s); }
    cleaned
}

pub fn apply(fragment: &str, flags: &CleanFlags) -> String {
    type Transform = fn(&str) -> Cow<'_, str>;
    
    let transforms: &[(bool, Transform)] = &[
        (flags.remove_rules, remove_horizontal_rules),
        (flags.remove_numbering, remove_list_numbering),
        (flags.remove_list_markers, remove_list_markers),
        (flags.remove_links, remove_links),
        (flags.format_latex, format_latex),
        (flags.remove_formatting, remove_markdown_formatting),
        (flags.strip_heading_markers, strip_heading_markers),
        (flags.compact_lines, remove_empty_lines),
    ];

    let mut c = Cow::Borrowed(fragment);
    
    for (enabled, transform) in transforms {
        if *enabled {
            if let Cow::Owned(s) = transform(&c) {
                c = Cow::Owned(s);
            }
        }
    }
    
    if flags.lower_case {
        c = Cow::Owned(c.to_lowercase());
    }
    
    c.trim().to_string()
}
