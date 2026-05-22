use crate::chunker::token::count_tokens;

pub fn split_by_words(text: &str, max_tokens: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut chunks = Vec::new();
    let mut current_chunk = Vec::new();
    let mut current_tokens = 0;

    for word in words {
        let word_tokens = count_tokens(word);
        if current_tokens + word_tokens > max_tokens && !current_chunk.is_empty() {
            chunks.push(current_chunk.join(" "));
            current_chunk = vec![word];
            current_tokens = word_tokens;
        } else {
            current_chunk.push(word);
            current_tokens += word_tokens;
        }
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk.join(" "));
    }
    chunks
}
