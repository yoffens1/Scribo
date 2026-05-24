use scribo_lib::chunker::stages::latex::*;

#[test]
fn test_format_latex_symbols() {
    // Inline symbols
    assert_eq!(format_latex("Let $ \\alpha + \\beta = \\gamma $ inline"), "Let $ α + β = γ $ inline");
    
    // Block symbols
    assert_eq!(format_latex("$$\n\\Gamma \\pm \\Delta\n$$"), "$$\nΓ ± Δ\n$$");
}

#[test]
fn test_format_latex_functions() {
    assert_eq!(format_latex("$\\sum_{i=1}^{n}$"), "$∑_i=1^n$");
    assert_eq!(format_latex("$\\vec{x} \\hat{y} \\bar{z} \\tilde{w}$"), "$x⃗ ŷ z̅ w̃$");
    assert_eq!(format_latex("$\\frac{a}{b}$"), "$(a)/(b)$");
}

#[test]
fn test_remove_latex() {
    let input = "Normal text with $ \\alpha $ math and $$\n\\sum\n$$ block math.";
    assert_eq!(remove_latex(input), "Normal text with  math and  block math.");
}
