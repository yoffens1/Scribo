use scribo_lib::chunker::types::{ChunkOptions, ChunkMode};
use scribo_lib::chunker::pipeline::chunk_for_embedding;
use scribo_lib::ai::embedding::Embedder;
use scribo_lib::ai::types::EmbedderConfig;
use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Использование: {} <путь_до_md_файла> [имя_модели]", args[0]);
        eprintln!("Пример: {} ~/Documents/Scribo/inbox/test.md nomic-embed-text", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];
    let content = std::fs::read_to_string(file_path).unwrap_or_else(|e| {
        eprintln!("Ошибка чтения файла '{}': {}", file_path, e);
        std::process::exit(1);
    });

    let options = ChunkOptions::default().for_mode(ChunkMode::Embedding);
    println!("Чанкинг файла '{}' с настройками Embedding...", file_path);
    
    let chunks = chunk_for_embedding(&content, &options);
    println!("Получено чанков: {}", chunks.len());

    let model_id = args.get(2).cloned().unwrap_or_else(|| "nomic-embed-text".to_string());

    let embedder_config = EmbedderConfig {
        provider: "local".to_string(),
        model: Some(model_id.clone()),
        api_key: None,
        base_url: None,
    };
    
    println!("Инициализация Embedder с локальной моделью '{}'...", model_id);
    let embedder = Embedder::new(embedder_config);

    for (i, chunk) in chunks.iter().enumerate() {
        println!("\n================ ЧАНК {} ================", i + 1);
        println!("{}\n------------------------------------------", chunk);
        
        match embedder.embed(chunk).await {
            Ok(vec) => {
                println!("Размерность вектора: {}", vec.len());
                let preview: Vec<f32> = vec.into_iter().take(15).collect();
                println!("Вектор (первые 15 значений): {:?}", preview);
            }
            Err(e) => {
                eprintln!("Ошибка получения вектора для чанка {}: {}", i + 1, e);
            }
        }
    }
}
