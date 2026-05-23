use regex::Regex;
use crate::chunker::token::count_tokens;
use crate::chunker::token::split_by_words;
use std::sync::LazyLock;

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
