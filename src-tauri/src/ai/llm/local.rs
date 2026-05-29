use async_trait::async_trait;
use reqwest::Client;
use llama_cpp_2::{
    model::{LlamaModel, params::LlamaModelParams, AddBos},
    context::params::LlamaContextParams,
    llama_batch::LlamaBatch,
    sampling::LlamaSampler,
};
use crate::ai::types::{LlmConfig, LlmResponse, Message};
use super::provider::LlmProvider;
use tauri::{AppHandle, Emitter};

pub struct LocalProvider;

#[async_trait]
impl LlmProvider for LocalProvider {
    fn name(&self) -> &'static str { "local" }

    async fn generate(
        &self,
        _client: &Client,
        config: &LlmConfig,
        messages: Vec<Message>,
        app: Option<AppHandle>,
    ) -> Result<LlmResponse, String> {
        let model_id = config.model.clone();
        let temperature = config.temperature;
        let max_tokens = config.max_tokens;

        // Получаем глобальный менеджер моделей
        let manager = crate::ai::models::manager::get_model_manager();
        
        // Загружаем модель (или берем из кэша)
        let loaded_model = manager.load_model(&model_id)?;
        let engine = loaded_model.engine.clone();

        let content = tokio::task::spawn_blocking(move || {
            engine.generate_sync(&messages, temperature, max_tokens, app)
        }).await.map_err(|e| e.to_string())??;

        Ok(LlmResponse {
            text: content,
            model: model_id,
            latency_ms: 0,
            prompt_tokens: Some(0),
            completion_tokens: Some(0),
            cached_tokens: Some(0),
        })
    }
}

/// Структура локальной модели (обертка над LlamaModel)
pub struct LocalLlm {
    pub model: LlamaModel,
}

impl LocalLlm {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let backend = crate::ai::models::manager::get_backend()?;
        
        let n_gpu_layers = if cfg!(any(target_os = "ios", target_os = "android")) {
            0
        } else {
            999
        };

        let params = LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers);
        let model = LlamaModel::load_from_file(backend, path, &params).map_err(|e| e.to_string())?;
        
        Ok(Self {
            model,
        })
    }

    pub fn generate_sync(&self, messages: &[Message], _temperature: Option<f32>, max_tokens: Option<u32>, app: Option<AppHandle>) -> Result<String, String> {
        let mut prompt = String::new();
        for msg in messages {
            prompt.push_str(&format!("<|im_start|>{}\n{}<|im_end|>\n", msg.role, msg.content));
        }
        prompt.push_str("<|im_start|>assistant\n");

        let backend = crate::ai::models::manager::get_backend()?;
        let ctx_params = LlamaContextParams::default().with_n_ctx(Some(std::num::NonZeroU32::new(crate::constants::EMBEDDING_CTX as u32).unwrap()));
        let mut ctx = self.model.new_context(backend, ctx_params).map_err(|e| e.to_string())?;

        let tokens_list = self.model.str_to_token(&prompt, AddBos::Always).map_err(|e| e.to_string())?;

        let mut batch = LlamaBatch::new(tokens_list.len(), 1);
        let last_index = tokens_list.len() - 1;
        for (i, token) in (0_i32..).zip(tokens_list.into_iter()) {
            let is_last = i as usize == last_index;
            batch.add(token, i, &[0], is_last).map_err(|e| e.to_string())?;
        }

        ctx.decode(&mut batch).map_err(|e| e.to_string())?;

        let mut response = String::new();
        
        let mut sampler = LlamaSampler::greedy();
        let mut n_cur = batch.n_tokens();
        let target = batch.n_tokens() + max_tokens.unwrap_or(crate::constants::DEFAULT_LLM_MAX_TOKENS) as i32;

        while n_cur < target {
            let new_token = sampler.sample(&ctx, batch.n_tokens() - 1);
            sampler.accept(new_token);

            if self.model.is_eog_token(new_token) {
                break;
            }

            let token_bytes = self.model.token_to_piece_bytes(new_token, 256, false, None).map_err(|e| e.to_string())?;
            let token_str = String::from_utf8_lossy(&token_bytes);
            response.push_str(&token_str);

            if let Some(app_handle) = &app {
                let _ = app_handle.emit("llm-token", token_str.to_string());
            }

            batch.clear();
            batch.add(new_token, n_cur, &[0], true).map_err(|e| e.to_string())?;
            ctx.decode(&mut batch).map_err(|e| e.to_string())?;
            n_cur += 1;
        }

        Ok(response)
    }

    pub fn embed_sync(&self, text: &str) -> Result<Vec<f32>, String> {
        let backend = crate::ai::models::manager::get_backend()?;
        let n_ctx_val = self.model.n_ctx_train() as u32;
        let n_ctx_val = if n_ctx_val > 0 { n_ctx_val } else { 512 };
        let n_ctx_val = n_ctx_val.min(2048);

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(std::num::NonZeroU32::new(n_ctx_val).unwrap()))
            .with_n_batch(n_ctx_val)
            .with_n_ubatch(n_ctx_val)
            .with_embeddings(true);
        let mut ctx = self.model.new_context(backend, ctx_params).map_err(|e| e.to_string())?;

        let mut tokens_list = self.model.str_to_token(text, AddBos::Always).map_err(|e| e.to_string())?;
        if tokens_list.len() > n_ctx_val as usize {
            tokens_list.truncate(n_ctx_val as usize);
        }

        let mut batch = LlamaBatch::new(tokens_list.len(), 1);
        let last_index = tokens_list.len() - 1;
        for (i, token) in (0_i32..).zip(tokens_list.into_iter()) {
            let is_last = i as usize == last_index;
            batch.add(token, i, &[0], is_last).map_err(|e| e.to_string())?;
        }

        ctx.decode(&mut batch).map_err(|e| e.to_string())?;

        let emb = ctx.embeddings_seq_ith(0).map_err(|e| e.to_string())?;

        Ok(emb.to_vec())
    }
}
