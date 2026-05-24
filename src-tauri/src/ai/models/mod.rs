use std::path::PathBuf;

pub mod scanner;
pub mod manager;

/// Возвращает путь к директории, в которой хранятся скачанные локальные модели.
/// Для десктопа: `~/.local/share/scribo/models` (Linux) или `~/Library/Application Support/scribo/models` (Mac)
/// Для мобилок: Внутренняя или доступная пользователю директория
pub fn models_dir() -> PathBuf {
    let base = dirs::data_dir()
        .or_else(|| dirs::home_dir())
        .unwrap_or_else(|| PathBuf::from("."));
    
    let dir = base.join("scribo").join("models");
    
    // Пытаемся создать директорию, если её нет
    if !dir.exists() {
        std::fs::create_dir_all(&dir).ok();
    }
    
    dir
}
