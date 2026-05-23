pub fn count_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    let words = text.split_whitespace().count();
    
    // Fast, conservative heuristic to avoid loading the 1.5MB cl100k_base dictionary:
    // - English: ~4 chars per token, ~1.3 tokens per word
    // - Cyrillic/Mixed: tokens can be closer to 2-3 chars per token
    // We take the maximum of both estimations and add a small buffer to safely overestimate
    // and never exceed LLM context windows.
    let estimate = (words as f64 * 1.5).max(chars as f64 / 3.0);
    
    estimate.ceil() as usize
}
