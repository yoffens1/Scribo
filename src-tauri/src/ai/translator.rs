use crate::ai::{LlmService, Message};
use std::sync::Arc;

pub struct Translator {
    llm: Arc<LlmService>,
}

impl Translator {
    pub fn new(llm: Arc<LlmService>) -> Self {
        Self { llm }
    }

    pub async fn translate(&self, text: &str, target_lang: &str) -> Result<String, String> {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: format!("You are a professional translator. Translate the following text into {} preserving markdown formatting, tone, and specific terminology.", target_lang),
            },
            Message {
                role: "user".to_string(),
                content: text.to_string(),
            },
        ];

        let response = self.llm.generate_messages(messages).await?;
        Ok(response.text)
    }
}
