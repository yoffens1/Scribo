use std::path::PathBuf;
use serde::Serialize;
use walkdir::WalkDir;
use std::io::Read;

#[derive(Serialize, Clone, Debug)]
pub enum ModelKind {
    Llm,
    Embedding,
}

#[derive(Serialize, Clone, Debug)]
pub struct LocalModel {
    pub id: String,                  
    pub path: PathBuf,               
    pub size_bytes: u64,             
    pub architecture: Option<String>,
    pub context_length: Option<u32>, 
    pub quantization: Option<String>,
    pub kind: ModelKind,             
}

/// Сканирует директорию с моделями и возвращает список доступных GGUF файлов
pub fn scan_models() -> Vec<LocalModel> {
    let dir = super::models_dir();
    let mut models = Vec::new();

    for entry in WalkDir::new(&dir).max_depth(2) {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) != Some("gguf") {
            continue;
        }

        let meta = std::fs::metadata(path).ok();
        let size_bytes = meta.map(|m| m.len()).unwrap_or(0);
        let id = match path.file_stem() {
            Some(stem) => stem.to_string_lossy().to_string(),
            None => continue,
        };

        let mut architecture = None;
        let mut context_length = None;
        let quantization = extract_quantization_from_filename(&id);

        // Парсинг GGUF шапки для получения метаданных
        if let Ok(mut f) = std::fs::File::open(path) {
            // Читаем первые 2MB — этого обычно хватает для всей шапки с запасом
            let mut buf = vec![0u8; 2 * 1024 * 1024]; 
            if let Ok(n) = f.read(&mut buf) {
                if let Ok(Some(gguf_file)) = gguf::GGUFFile::read(&buf[..n]) {
                    for m in &gguf_file.header.metadata {
                        if m.key == "general.architecture" {
                            if let gguf::GGUFMetadataValue::String(ref s) = m.value {
                                architecture = Some(s.clone());
                            }
                        } else if m.key.ends_with(".context_length") {
                            match m.value {
                                gguf::GGUFMetadataValue::Uint32(v) => context_length = Some(v),
                                gguf::GGUFMetadataValue::Int32(v) => context_length = Some(v as u32),
                                gguf::GGUFMetadataValue::Uint64(v) => context_length = Some(v as u32),
                                _ => {}
                            }
                        } else if m.key == "general.quantization_version" {
                            // Игнорируем, если уже нашли в имени файла
                        }
                    }
                }
            }
        }

        let kind = if id.to_lowercase().contains("embed") || id.to_lowercase().contains("nomic") {
            ModelKind::Embedding
        } else {
            ModelKind::Llm
        };

        models.push(LocalModel {
            id,
            path: path.to_path_buf(),
            size_bytes,
            architecture,
            context_length,
            quantization,
            kind,
        });
    }
    
    models
}

fn extract_quantization_from_filename(filename: &str) -> Option<String> {
    let lower = filename.to_lowercase();
    let parts: Vec<&str> = lower.split(|c: char| c == '-' || c == '_').collect();
    for part in parts {
        if part.starts_with('q') && part.len() >= 2 && part.len() <= 6 {
            // e.g. q4_k_m, q8_0
            return Some(part.to_uppercase());
        }
    }
    None
}
