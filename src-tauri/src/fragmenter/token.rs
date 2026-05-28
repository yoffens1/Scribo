use std::sync::LazyLock;
use regex::Regex;

pub fn count_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    let words = text.split_whitespace().count();
    
    // Fast, conservative heuristic to avoid loading the 1.5MB cl100k_base dictionary
    let estimate = (words as f64 * 1.5).max(chars as f64 / 3.0);
    
    estimate.ceil() as usize
}

static RE_SENTENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\.!\?]+[\.!\?]+|.+$").unwrap());

pub fn split_by_sentence_boundaries(text: &str, max_tokens: usize) -> Vec<(String, usize)> {
    let sentences: Vec<&str> = RE_SENTENCE.find_iter(text).map(|m| m.as_str()).collect();
    if sentences.is_empty() {
        return split_by_words(text, max_tokens);
    }

    let mut parts = Vec::new();
    let mut current = String::new();
    let mut current_tokens = 0;

    for sentence in sentences {
        let trimmed = sentence.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        let sentence_tokens = count_tokens(trimmed);
        let separator_tokens = if current.is_empty() { 0 } else { 1 };

        if current.is_empty() {
            if sentence_tokens <= max_tokens {
                current = trimmed.to_string();
                current_tokens = sentence_tokens;
            } else {
                parts.extend(split_by_words(trimmed, max_tokens));
            }
        } else if current_tokens + separator_tokens + sentence_tokens <= max_tokens {
            current = format!("{} {}", current, trimmed);
            current_tokens += separator_tokens + sentence_tokens;
        } else {
            parts.push((current.trim().to_string(), current_tokens));
            if sentence_tokens > max_tokens {
                parts.extend(split_by_words(trimmed, max_tokens));
                current = String::new();
                current_tokens = 0;
            } else {
                current = trimmed.to_string();
                current_tokens = sentence_tokens;
            }
        }
    }

    if !current.is_empty() {
        parts.push((current.trim().to_string(), current_tokens));
    }

    if parts.is_empty() {
        split_by_words(text, max_tokens)
    } else {
        parts
    }
}

pub fn split_by_words(text: &str, max_tokens: usize) -> Vec<(String, usize)> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut fragments = Vec::new();
    let mut current_fragment = Vec::new();
    let mut current_len = 0;
    let mut current_tokens = 0;
    let mut exact_mode = false;

    // A conservative heuristic: average 2 bytes per token.
    let safe_len = max_tokens * 2;

    for word in words {
        let word_len = word.len();

        if !exact_mode && current_len + word_len + 1 > safe_len && !current_fragment.is_empty() {
            exact_mode = true;
            let joined = current_fragment.join(" ");
            current_tokens = count_tokens(&joined);
        }

        if exact_mode {
            let word_tokens = count_tokens(word);
            let separator_tokens = if current_fragment.is_empty() { 0 } else { 1 };
            
            if current_tokens + separator_tokens + word_tokens > max_tokens && !current_fragment.is_empty() {
                let joined = current_fragment.join(" ");
                fragments.push((joined, current_tokens));
                
                current_fragment = vec![word];
                current_len = word_len;
                current_tokens = word_tokens;
                exact_mode = false;
                continue;
            } else {
                current_tokens += separator_tokens + word_tokens;
            }
        }

        current_fragment.push(word);
        current_len += word_len + 1;
    }

    if !current_fragment.is_empty() {
        let joined = current_fragment.join(" ");
        let tokens = if exact_mode { current_tokens } else { count_tokens(&joined) };
        fragments.push((joined, tokens));
    }
    fragments
}

pub fn split_oversized_paragraph(para: &str, max_tokens: usize) -> Vec<(String, usize)> {
    let lines: Vec<&str> = para.split('\n').collect();
    let mut fragments = Vec::new();
    let mut batch = Vec::new();
    let mut batch_tokens = 0;

    for line in lines {
        let lt = count_tokens(line);

        if lt > max_tokens {
            if !batch.is_empty() {
                fragments.push((batch.join("\n"), batch_tokens));
                batch.clear();
                batch_tokens = 0;
            }
            fragments.extend(split_by_sentence_boundaries(line, max_tokens));
            continue;
        }

        let separator_tokens = if !batch.is_empty() { 1 } else { 0 };
        if batch_tokens + separator_tokens + lt > max_tokens && !batch.is_empty() {
            fragments.push((batch.join("\n"), batch_tokens));
            batch.clear();
            batch_tokens = 0;
        }

        batch.push(line);
        batch_tokens += separator_tokens + lt;
    }

    if !batch.is_empty() {
        fragments.push((batch.join("\n"), batch_tokens));
    }

    if fragments.is_empty() {
        vec![(para.to_string(), count_tokens(para))]
    } else {
        fragments
    }
}
