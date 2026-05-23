use crate::ai::types::Message;

pub fn build_hyde_prompt(query: &str, lang: &str) -> Vec<Message> {
    vec![
        Message {
            role: "system".to_string(),
            content: format!("Write a concise, factual answer to the query in {}. Be informative and specific.", lang),
        },
        Message {
            role: "user".to_string(),
            content: query.to_string(),
        }
    ]
}

pub fn build_synonym_expansion_prompt(query: &str, max_synonyms: usize, lang: &str) -> Vec<Message> {
    vec![
        Message {
            role: "system".to_string(),
            content: format!(
                "Generate {} alternative search queries that express the same information need in {}. Return ONLY a JSON object: {{\"synonyms\": [\"query1\", \"query2\", ...]}}.",
                max_synonyms, lang
            ),
        },
        Message {
            role: "user".to_string(),
            content: query.to_string(),
        }
    ]
}

pub fn build_rerank_listwise_prompt(query: &str, candidates: &[(usize, String)]) -> Vec<Message> {
    let mut passages = String::new();
    for (i, text) in candidates {
        passages.push_str(&format!("[{}] {}\n\n", i, text.chars().take(500).collect::<String>()));
    }

    vec![
        Message {
            role: "system".to_string(),
            content: "Sort the following passages by relevance to the query. Return ONLY a JSON object: {\"order\": [3, 0, 5, 1, ...]}. Passages not in output are assumed irrelevant.".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: format!("Query: {}\n\nPassages:\n{}", query, passages),
        }
    ]
}

pub fn build_rerank_scoring_prompt(query: &str, candidates: &[(usize, String)]) -> Vec<Message> {
    let mut passages = String::new();
    for (i, text) in candidates {
        passages.push_str(&format!("[{}] {}\n\n", i, text.chars().take(500).collect::<String>()));
    }

    vec![
        Message {
            role: "system".to_string(),
            content: "Rate relevance of each passage to the query on a scale 0–10. Return ONLY a JSON array: [{\"id\": 0, \"score\": 7.5}, ...]. No explanation.".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: format!("Query: {}\n\nPassages:\n{}", query, passages),
        }
    ]
}
