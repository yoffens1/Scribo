use crate::ai::LlmService;
use crate::translation::Translator;
use std::sync::Arc;

pub async fn run_translation(
    llm: &Arc<LlmService>,
    query: &str,
    target_lang: &str,
) -> Option<String> {
    let translator = Translator::new(Arc::clone(llm));
    translator.translate(query, target_lang).await.ok()
}
