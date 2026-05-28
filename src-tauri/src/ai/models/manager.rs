use std::sync::{Arc, OnceLock};
use parking_lot::RwLock;
use lru::LruCache;
use std::num::NonZeroUsize;
use llama_cpp_2::llama_backend::LlamaBackend;
use crate::ai::llm::local::LocalLlm;

pub struct LoadedModel {
    pub id: String,
    pub engine: Arc<LocalLlm>,
}

pub struct ModelManager {
    // LRU Cache, ограниченный двумя загруженными моделями (LLM + Embedding)
    active_models: RwLock<LruCache<String, Arc<LoadedModel>>>,
    loading_lock: parking_lot::Mutex<()>,
}

static GLOBAL_MODEL_MANAGER: OnceLock<ModelManager> = OnceLock::new();
static GLOBAL_BACKEND: OnceLock<Result<LlamaBackend, String>> = OnceLock::new();

pub fn get_backend() -> Result<&'static LlamaBackend, String> {
    GLOBAL_BACKEND
        .get_or_init(|| LlamaBackend::init().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| e.clone())
}

pub fn get_model_manager() -> &'static ModelManager {
    GLOBAL_MODEL_MANAGER.get_or_init(|| ModelManager::new())
}

impl ModelManager {
    pub fn new() -> Self {
        Self {
            active_models: RwLock::new(LruCache::new(NonZeroUsize::new(2).unwrap())),
            loading_lock: parking_lot::Mutex::new(()),
        }
    }

    pub fn load_model(&self, id: &str) -> Result<Arc<LoadedModel>, String> {
        // Быстрый путь: проверяем кэш
        {
            let mut cache = self.active_models.write();
            if let Some(model) = cache.get(id) {
                return Ok(model.clone());
            }
        }

        // Сериализуем медленные загрузки для предотвращения параллельной двойной загрузки одной модели
        let _guard = self.loading_lock.lock();

        // Повторно проверяем кэш после получения блокировки
        {
            let mut cache = self.active_models.write();
            if let Some(model) = cache.get(id) {
                return Ok(model.clone());
            }
        }

        // Загружаем
        let mut path = super::models_dir();
        path.push(format!("{}.gguf", id));
        
        let engine = LocalLlm::load(&path)?;
        
        let new_model = Arc::new(LoadedModel {
            id: id.to_string(),
            engine: Arc::new(engine),
        });

        // Добавляем в кэш. Старая модель будет автоматически вытеснена и уничтожена
        let mut cache = self.active_models.write();
        cache.put(id.to_string(), new_model.clone());

        Ok(new_model)
    }

    pub fn unload_model(&self, id: &str) {
        let mut cache = self.active_models.write();
        cache.pop(id);
    }

    pub fn clear(&self) {
        let mut cache = self.active_models.write();
        cache.clear();
    }
}
