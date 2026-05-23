use crate::chunker::token::count_tokens;
use crate::chunker::token::split_by_sentence_boundaries;

pub fn split_oversized_paragraph(para: &str, max_tokens: usize) -> Vec<(String, usize)> {
    let lines: Vec<&str> = para.split('\n').collect();
    let mut chunks = Vec::new();
    let mut batch = Vec::new();
    let mut batch_tokens = 0;

    for line in lines {
        let lt = count_tokens(line);

        if lt > max_tokens {
            if !batch.is_empty() {
                chunks.push((batch.join("\n"), batch_tokens));
                batch.clear();
                batch_tokens = 0;
            }
            chunks.extend(split_by_sentence_boundaries(line, max_tokens));
            continue;
        }

        let separator_tokens = if !batch.is_empty() { 1 } else { 0 };
        if batch_tokens + separator_tokens + lt > max_tokens && !batch.is_empty() {
            chunks.push((batch.join("\n"), batch_tokens));
            batch.clear();
            batch_tokens = 0;
        }

        batch.push(line);
        batch_tokens += separator_tokens + lt;
    }

    if !batch.is_empty() {
        chunks.push((batch.join("\n"), batch_tokens));
    }

    if chunks.is_empty() {
        vec![(para.to_string(), count_tokens(para))]
    } else {
        chunks
    }
}
