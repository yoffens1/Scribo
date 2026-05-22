/**
 * Converts LaTeX expressions into human‑readable UTF‑8 text.
 * - $...$ / $$...$$ wrappers are stripped.
 * - Common commands are replaced with Unicode symbols.
 * - Subscripts / superscripts are kept as _{…} / ^{…}.
 * - Fractions become (a)/(b).
 */

export function formatLatex(text: string): string {
  let cleaned = text.replace(/\$\$([\s\S]*?)\$\$/g, "$1"); // display math
  cleaned = cleaned.replace(/\$([^$]+)\$/g, "$1");          // inline math

  cleaned = cleaned

    // ── Lowercase Greek ────────────────────────────────────────────
    .replace(/\\alpha\b/g, "α")
    .replace(/\\beta\b/g, "β")
    .replace(/\\gamma\b/g, "γ")
    .replace(/\\delta\b/g, "δ")
    .replace(/\\epsilon\b/g, "ε")
    .replace(/\\zeta\b/g, "ζ")
    .replace(/\\eta\b/g, "η")
    .replace(/\\theta\b/g, "θ")
    .replace(/\\iota\b/g, "ι")
    .replace(/\\kappa\b/g, "κ")
    .replace(/\\lambda\b/g, "λ")
    .replace(/\\mu\b/g, "μ")
    .replace(/\\nu\b/g, "ν")
    .replace(/\\xi\b/g, "ξ")
    .replace(/\\pi\b/g, "π")
    .replace(/\\rho\b/g, "ρ")
    .replace(/\\sigma\b/g, "σ")
    .replace(/\\tau\b/g, "τ")
    .replace(/\\upsilon\b/g, "υ")
    .replace(/\\phi\b/g, "φ")
    .replace(/\\chi\b/g, "χ")
    .replace(/\\psi\b/g, "ψ")
    .replace(/\\omega\b/g, "ω")

    // ── Uppercase Greek ────────────────────────────────────────────
    .replace(/\\Gamma\b/g, "Γ")
    .replace(/\\Delta\b/g, "Δ")
    .replace(/\\Theta\b/g, "Θ")
    .replace(/\\Lambda\b/g, "Λ")
    .replace(/\\Xi\b/g, "Ξ")
    .replace(/\\Pi\b/g, "Π")
    .replace(/\\Sigma\b/g, "Σ")
    .replace(/\\Upsilon\b/g, "Υ")
    .replace(/\\Phi\b/g, "Φ")
    .replace(/\\Psi\b/g, "Ψ")
    .replace(/\\Omega\b/g, "Ω")

    // ── Logic operators ────────────────────────────────────────────
    .replace(/\\forall(?![a-zA-Z])/g, "∀")       // universal quantifier
    .replace(/\\exists(?![a-zA-Z])/g, "∃")       // existential quantifier
    .replace(/\\neg(?![a-zA-Z])|\\lnot(?![a-zA-Z])/g, "¬")   // negation
    .replace(/\\land(?![a-zA-Z])|\\wedge(?![a-zA-Z])/g, "∧") // and
    .replace(/\\lor(?![a-zA-Z])|\\vee(?![a-zA-Z])/g, "∨")    // or
    .replace(/\\rightarrow(?![a-zA-Z])|\\to(?![a-zA-Z])/g, "→")       // implication
    .replace(/\\leftrightarrow(?![a-zA-Z])/g, "↔")                     // iff
    .replace(/\\Rightarrow(?![a-zA-Z])/g, "⇒")                         // double arrow
    .replace(/\\Leftrightarrow(?![a-zA-Z])/g, "⇔")                     // double iff
    .replace(/\\top(?![a-zA-Z])/g, "⊤")                                // top / true
    .replace(/\\bot(?![a-zA-Z])/g, "⊥")                                // bottom / false
    .replace(/\\vdash(?![a-zA-Z])/g, "⊢")                              // entails
    .replace(/\\models(?![a-zA-Z])/g, "⊨")                             // models
    .replace(/\\equiv(?![a-zA-Z])/g, "≡")                              // equivalence

    // ── Large operators (with subscript / superscript) ─────────────
    .replace(/\\sum\s*_\{([^}]+)\}\s*\^\{([^}]+)\}/g, "∑_{$1}^{$2}")
    .replace(/\\sum\s*_\{([^}]+)\}/g, "∑_{$1}")
    .replace(/\\sum\s*\^\{([^}]+)\}/g, "∑^{$1}")
    .replace(/\\prod\s*_\{([^}]+)\}\s*\^\{([^}]+)\}/g, "∏_{$1}^{$2}")
    .replace(/\\prod\s*_\{([^}]+)\}/g, "∏_{$1}")
    .replace(/\\prod\s*\^\{([^}]+)\}/g, "∏^{$1}")

    // ── Plain large operators (no limits) ──────────────────────────
    .replace(/\\sum(?![a-zA-Z])/g, "∑")
    .replace(/\\prod(?![a-zA-Z])/g, "∏")
    .replace(/\\int(?![a-zA-Z])/g, "∫")
    .replace(/\\infty(?![a-zA-Z])/g, "∞")

    // ── Arithmetic / binary operators ──────────────────────────────
    .replace(/\\cdot(?![a-zA-Z])/g, "·")
    .replace(/\\times(?![a-zA-Z])/g, "×")
    .replace(/\\div(?![a-zA-Z])/g, "÷")
    .replace(/\\pm(?![a-zA-Z])/g, "±")
    .replace(/\\mp(?![a-zA-Z])/g, "∓")
    .replace(/\\ast(?![a-zA-Z])/g, "*")

    // ── Relations ──────────────────────────────────────────────────
    .replace(/\\leq(?![a-zA-Z])/g, "≤")
    .replace(/\\geq(?![a-zA-Z])/g, "≥")
    .replace(/\\neq(?![a-zA-Z])/g, "≠")
    .replace(/\\approx(?![a-zA-Z])/g, "≈")
    .replace(/\\propto(?![a-zA-Z])/g, "∝")
    .replace(/\\sim(?![a-zA-Z])/g, "∼")

    // ── Arrows ─────────────────────────────────────────────────────
    .replace(/\\leftarrow(?![a-zA-Z])/g, "←")
    .replace(/\\Rightarrow(?![a-zA-Z])/g, "⇒")
    .replace(/\\Leftrightarrow(?![a-zA-Z])/g, "⇔")

    // ── Accents (vec, hat, tilde, bar) ─────────────────────────────
    .replace(/\\vec\{([^}]+)\}/g, "$1⃗")
    .replace(/\\hat\{([^}]+)\}/g, "$1̂")
    .replace(/\\tilde\{([^}]+)\}/g, "$1̃")
    .replace(/\\bar\{([^}]+)\}/g, "$1̅")

    // ── Fractions ──────────────────────────────────────────────────
    .replace(/\\frac\{([^}]+)\}\{([^}]+)\}/g, "($1)/($2)")

    // ── Final cleanup ──────────────────────────────────────────────
    .replace(/[{}]/g, "")              // remove stray braces
    .replace(/\\[a-zA-Z]+/g, "");       // drop remaining unknown commands

  return cleaned.trim();
}
