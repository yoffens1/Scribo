use crate::ai::types::Message;

pub fn build_translate_prompt(text: &str, tgt: &str) -> Vec<Message> {
    vec![
        Message {
            role: "system".to_string(),
            content: format!("You are a translator. Translate the user text to {}.\nRules:\n- Translate EVERY word. Do NOT leave any English words.\n- Output ONLY the translated text. No quotes, no explanations, no prefix.\n- Preserve question marks and punctuation.", tgt),
        },
        Message {
            role: "user".to_string(),
            content: text.to_string(),
        }
    ]
}

pub fn build_translate_strict_prompt(text: &str, tgt: &str) -> Vec<Message> {
    vec![
        Message {
            role: "system".to_string(),
            content: format!("Translate to {}. Use ONLY {} language characters.\nNo English words allowed in output. No explanations.", tgt, tgt),
        },
        Message {
            role: "user".to_string(),
            content: text.to_string(),
        }
    ]
}
