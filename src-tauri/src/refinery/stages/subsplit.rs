use crate::refinery::types::AtomChunk;

pub async fn run_subsplit_stage(chunks: Vec<AtomChunk>) -> Vec<AtomChunk> {
    let mut result = Vec::new();
    let max_limit = 1000;
    
    for chunk in chunks {
        // Approximate token count
        let tokens = chunk.generation_text.split_whitespace().count() * 4 / 3;
        
        if tokens <= max_limit {
            result.push(chunk);
            continue;
        }

        let sentences: Vec<&str> = chunk.generation_text.split_terminator(|c| c == '.' || c == '!' || c == '?').collect();
        if sentences.len() <= 1 {
            result.push(chunk);
            continue;
        }

        let mut current_sentences = Vec::new();
        let mut current_tokens = 0;
        let mut part = 0;

        for sentence in sentences {
            let sentence = sentence.trim();
            if sentence.is_empty() { continue; }
            let s_tokens = sentence.split_whitespace().count() * 4 / 3;

            if current_tokens + s_tokens > max_limit && !current_sentences.is_empty() {
                let split_text = current_sentences.join(" ") + ".";
                let mut new_chunk = chunk.clone();
                new_chunk.hash = format!("{}-s{}", chunk.hash, part);
                new_chunk.embedding_text = split_text.clone();
                new_chunk.generation_text = split_text;
                new_chunk.index = chunk.index + part;
                result.push(new_chunk);
                
                part += 1;
                current_sentences = vec![sentence];
                current_tokens = s_tokens;
            } else {
                current_sentences.push(sentence);
                current_tokens += s_tokens;
            }
        }

        if !current_sentences.is_empty() {
            let split_text = current_sentences.join(" ") + ".";
            let mut new_chunk = chunk.clone();
            new_chunk.hash = format!("{}-s{}", chunk.hash, part);
            new_chunk.embedding_text = split_text.clone();
            new_chunk.generation_text = split_text;
            new_chunk.index = chunk.index + part;
            result.push(new_chunk);
        }
    }
    result
}
