use crate::chunker::token::count_tokens;

pub fn split_by_words(text: &str, max_tokens: usize) -> Vec<(String, usize)> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut chunks = Vec::new();
    let mut current_chunk = Vec::new();
    let mut current_len = 0;
    let mut current_tokens = 0;
    let mut exact_mode = false;

    // A conservative heuristic: average 2 bytes per token.
    let safe_len = max_tokens * 2;

    for word in words {
        let word_len = word.len();

        if !exact_mode && current_len + word_len + 1 > safe_len && !current_chunk.is_empty() {
            exact_mode = true;
            let joined = current_chunk.join(" ");
            current_tokens = count_tokens(&joined);
        }

        if exact_mode {
            let word_tokens = count_tokens(word);
            let separator_tokens = if current_chunk.is_empty() { 0 } else { 1 };
            
            if current_tokens + separator_tokens + word_tokens > max_tokens && !current_chunk.is_empty() {
                let joined = current_chunk.join(" ");
                chunks.push((joined, current_tokens));
                
                current_chunk = vec![word];
                current_len = word_len;
                current_tokens = word_tokens;
                exact_mode = false;
                continue;
            } else {
                current_tokens += separator_tokens + word_tokens;
            }
        }

        current_chunk.push(word);
        current_len += word_len + 1;
    }

    if !current_chunk.is_empty() {
        let joined = current_chunk.join(" ");
        let tokens = if exact_mode { current_tokens } else { count_tokens(&joined) };
        chunks.push((joined, tokens));
    }
    chunks
}
