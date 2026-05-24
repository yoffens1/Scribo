use crate::ai::{LlmService, Message};
use super::prompts::translation;
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
                role: "system".into(),
                content: translation::system_prompt(target_lang),
            },
            Message {
                role: "user".into(),
                content: text.to_string(),
            },
        ];

        let response = self.llm.generate_messages(messages).await?;
        Ok(response.text)
    }
}
