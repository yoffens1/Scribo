use regex::Regex;

pub fn format_latex(text: &str) -> String {
    let re_block = Regex::new(r"\$\$([\s\S]*?)\$\$").unwrap();
    let mut cleaned = re_block.replace_all(text, "$1").to_string();
    
    let re_inline = Regex::new(r"\$([^$]+)\$").unwrap();
    cleaned = re_inline.replace_all(&cleaned, "$1").to_string();

    let replacements = [
        (r"\\alpha\b", "α"), (r"\\beta\b", "β"), (r"\\gamma\b", "γ"),
        (r"\\delta\b", "δ"), (r"\\epsilon\b", "ε"), (r"\\zeta\b", "ζ"),
        (r"\\eta\b", "η"), (r"\\theta\b", "θ"), (r"\\iota\b", "ι"),
        (r"\\kappa\b", "κ"), (r"\\lambda\b", "λ"), (r"\\mu\b", "μ"),
        (r"\\nu\b", "ν"), (r"\\xi\b", "ξ"), (r"\\pi\b", "π"),
        (r"\\rho\b", "ρ"), (r"\\sigma\b", "σ"), (r"\\tau\b", "τ"),
        (r"\\upsilon\b", "υ"), (r"\\phi\b", "φ"), (r"\\chi\b", "χ"),
        (r"\\psi\b", "ψ"), (r"\\omega\b", "ω"),
        
        (r"\\Gamma\b", "Γ"), (r"\\Delta\b", "Δ"), (r"\\Theta\b", "Θ"),
        (r"\\Lambda\b", "Λ"), (r"\\Xi\b", "Ξ"), (r"\\Pi\b", "Π"),
        (r"\\Sigma\b", "Σ"), (r"\\Upsilon\b", "Υ"), (r"\\Phi\b", "Φ"),
        (r"\\Psi\b", "Ψ"), (r"\\Omega\b", "Ω"),

        (r"\\forall\b", "∀"), (r"\\exists\b", "∃"),
        (r"\\neg\b|\\lnot\b", "¬"), (r"\\land\b|\\wedge\b", "∧"),
        (r"\\lor\b|\\vee\b", "∨"), (r"\\rightarrow\b|\\to\b", "→"),
        (r"\\leftrightarrow\b", "↔"), (r"\\Rightarrow\b", "⇒"),
        (r"\\Leftrightarrow\b", "⇔"), (r"\\top\b", "⊤"),
        (r"\\bot\b", "⊥"), (r"\\vdash\b", "⊢"),
        (r"\\models\b", "⊨"), (r"\\equiv\b", "≡"),

        (r"\\sum\b", "∑"), (r"\\prod\b", "∏"),
        (r"\\int\b", "∫"), (r"\\infty\b", "∞"),

        (r"\\cdot\b", "·"), (r"\\times\b", "×"),
        (r"\\div\b", "÷"), (r"\\pm\b", "±"),
        (r"\\mp\b", "∓"), (r"\\ast\b", "*"),

        (r"\\leq\b", "≤"), (r"\\geq\b", "≥"),
        (r"\\neq\b", "≠"), (r"\\approx\b", "≈"),
        (r"\\propto\b", "∝"), (r"\\sim\b", "∼"),

        (r"\\leftarrow\b", "←"),
    ];

    for (pattern, replacement) in replacements.iter() {
        let re = Regex::new(pattern).unwrap();
        cleaned = re.replace_all(&cleaned, *replacement).to_string();
    }

    let re_sum_sub_sup = Regex::new(r"\\sum\s*_\{([^}]+)\}\s*\^\{([^}]+)\}").unwrap();
    cleaned = re_sum_sub_sup.replace_all(&cleaned, "∑_{$1}^{$2}").to_string();
    let re_sum_sub = Regex::new(r"\\sum\s*_\{([^}]+)\}").unwrap();
    cleaned = re_sum_sub.replace_all(&cleaned, "∑_{$1}").to_string();
    let re_sum_sup = Regex::new(r"\\sum\s*\^\{([^}]+)\}").unwrap();
    cleaned = re_sum_sup.replace_all(&cleaned, "∑^{$1}").to_string();

    let re_prod_sub_sup = Regex::new(r"\\prod\s*_\{([^}]+)\}\s*\^\{([^}]+)\}").unwrap();
    cleaned = re_prod_sub_sup.replace_all(&cleaned, "∏_{$1}^{$2}").to_string();
    let re_prod_sub = Regex::new(r"\\prod\s*_\{([^}]+)\}").unwrap();
    cleaned = re_prod_sub.replace_all(&cleaned, "∏_{$1}").to_string();
    let re_prod_sup = Regex::new(r"\\prod\s*\^\{([^}]+)\}").unwrap();
    cleaned = re_prod_sup.replace_all(&cleaned, "∏^{$1}").to_string();

    let re_vec = Regex::new(r"\\vec\{([^}]+)\}").unwrap();
    cleaned = re_vec.replace_all(&cleaned, "$1⃗").to_string();
    let re_hat = Regex::new(r"\\hat\{([^}]+)\}").unwrap();
    cleaned = re_hat.replace_all(&cleaned, "$1̂").to_string();
    let re_tilde = Regex::new(r"\\tilde\{([^}]+)\}").unwrap();
    cleaned = re_tilde.replace_all(&cleaned, "$1̃").to_string();
    let re_bar = Regex::new(r"\\bar\{([^}]+)\}").unwrap();
    cleaned = re_bar.replace_all(&cleaned, "$1̅").to_string();

    let re_frac = Regex::new(r"\\frac\{([^}]+)\}\{([^}]+)\}").unwrap();
    cleaned = re_frac.replace_all(&cleaned, "($1)/($2)").to_string();

    let re_braces = Regex::new(r"[{}]").unwrap();
    cleaned = re_braces.replace_all(&cleaned, "").to_string();

    let re_unknown = Regex::new(r"\\[a-zA-Z]+").unwrap();
    cleaned = re_unknown.replace_all(&cleaned, "").to_string();

    cleaned.trim().to_string()
}
