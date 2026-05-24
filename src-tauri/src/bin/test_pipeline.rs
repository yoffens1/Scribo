use scribo_lib::fragmenter::types::{FragmentOptions, FragmentMode};
use scribo_lib::fragmenter::pipeline::fragment_for_embedding;
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

    let options = FragmentOptions::default().for_mode(FragmentMode::Embedding);
    println!("Фрагментирование файла '{}' с настройками Embedding...", file_path);
    
    let fragments = fragment_for_embedding(&content, &options);
    println!("Получено фрагментов: {}", fragments.len());

    let model_id = args.get(2).cloned().unwrap_or_else(|| "nomic-embed-text".to_string());

    let embedder_config = EmbedderConfig {
        provider: "local".to_string(),
        model: Some(model_id.clone()),
        api_key: None,
        base_url: None,
    };
    
    println!("Инициализация Embedder с локальной моделью '{}'...", model_id);
    let embedder = Embedder::new(embedder_config);

    for (i, fragment) in fragments.iter().enumerate() {
        println!("\n================ ФРАГМЕНТ {} ================", i + 1);
        println!("{}\n------------------------------------------", fragment);
        
        match embedder.embed(fragment).await {
            Ok(vec) => {
                println!("Размерность вектора: {}", vec.len());
                let preview: Vec<f32> = vec.into_iter().take(15).collect();
                println!("Вектор (первые 15 значений): {:?}", preview);
            }
            Err(e) => {
                eprintln!("Ошибка получения вектора для фрагмента {}: {}", i + 1, e);
            }
        }
    }
}
