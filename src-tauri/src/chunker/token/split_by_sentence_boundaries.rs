use regex::Regex;
use crate::chunker::token::count_tokens;
use crate::chunker::token::split_by_words;

pub fn split_by_sentence_boundaries(line: &str, max_tokens: usize) -> Vec<String> {
    let re = Regex::new(r"[^\.!\?]+[\.!\?]+|.+$").unwrap();
    let sentences: Vec<&str> = re.find_iter(line).map(|m| m.as_str()).collect();
    if sentences.is_empty() {
        return split_by_words(line, max_tokens);
    }

    let mut parts = Vec::new();
    let mut current = String::new();

    for sentence in sentences {
        let trimmed = sentence.trim();
        if trimmed.is_empty() {
            continue;
        }
        let candidate = if current.is_empty() {
            trimmed.to_string()
        } else {
            format!("{} {}", current, trimmed)
        };

        if count_tokens(&candidate) <= max_tokens {
            current = candidate;
        } else {
            if !current.is_empty() {
                parts.push(current.trim().to_string());
            }
            if count_tokens(trimmed) > max_tokens {
                parts.extend(split_by_words(trimmed, max_tokens));
                current = String::new();
            } else {
                current = trimmed.to_string();
            }
        }
    }

    if !current.is_empty() {
        parts.push(current.trim().to_string());
    }

    if parts.is_empty() {
        split_by_words(line, max_tokens)
    } else {
        parts
    }
}
