use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;
use regex::Regex;

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

fn chain<'a>(text: Cow<'a, str>, re: &Regex, rep: &str) -> Cow<'a, str> {
    match re.replace_all(&text, rep) {
        Cow::Owned(s) => Cow::Owned(s),
        Cow::Borrowed(_) => text,
    }
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

    cleaned = chain(cleaned, &RE_SUM_SUB_SUP, "∑_{$1}^{$2}");
    cleaned = chain(cleaned, &RE_SUM_SUB, "∑_{$1}");
    cleaned = chain(cleaned, &RE_SUM_SUP, "∑^{$1}");
    
    cleaned = chain(cleaned, &RE_PROD_SUB_SUP, "∏_{$1}^{$2}");
    cleaned = chain(cleaned, &RE_PROD_SUB, "∏_{$1}");
    cleaned = chain(cleaned, &RE_PROD_SUP, "∏^{$1}");

    cleaned = chain(cleaned, &RE_VEC, "$1⃗");
    cleaned = chain(cleaned, &RE_HAT, "$1̂");
    cleaned = chain(cleaned, &RE_TILDE, "$1̃");
    cleaned = chain(cleaned, &RE_BAR, "$1̅");
    cleaned = chain(cleaned, &RE_FRAC, "($1)/($2)");
    cleaned = chain(cleaned, &RE_BRACES, "");
    cleaned = chain(cleaned, &RE_UNKNOWN, "");

    cleaned.into_owned()
}

pub fn remove_latex(text: &str) -> Cow<'_, str> {
    let mut cleaned = RE_LATEX_BLOCK.replace_all(text, "");
    if let Cow::Owned(s) = RE_LATEX_INLINE.replace_all(&cleaned, "") {
        cleaned = Cow::Owned(s);
    }
    cleaned
}
