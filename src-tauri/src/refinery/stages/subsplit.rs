use crate::refinery::types::AtomFragment;

pub async fn run_subsplit_stage(fragments: Vec<AtomFragment>) -> Vec<AtomFragment> {
    let mut result = Vec::new();
    let max_limit = 1000;
    
    for fragment in fragments {
        // Approximate token count
        let tokens = fragment.generation_text.split_whitespace().count() * 4 / 3;
        
        if tokens <= max_limit {
            result.push(fragment);
            continue;
        }

        let sentences: Vec<&str> = fragment.generation_text.split_terminator(|c| c == '.' || c == '!' || c == '?').collect();
        if sentences.len() <= 1 {
            result.push(fragment);
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
                let mut new_fragment = fragment.clone();
                new_fragment.hash = format!("{}-s{}", fragment.hash, part);
                new_fragment.embedding_text = split_text.clone();
                new_fragment.generation_text = split_text;
                new_fragment.index = fragment.index + part;
                result.push(new_fragment);
                
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
            let mut new_fragment = fragment.clone();
            new_fragment.hash = format!("{}-s{}", fragment.hash, part);
            new_fragment.embedding_text = split_text.clone();
            new_fragment.generation_text = split_text;
            new_fragment.index = fragment.index + part;
            result.push(new_fragment);
        }
    }
    result
}
