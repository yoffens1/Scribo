use std::path::PathBuf;
use scribo_lib::chunker::{chunk_paired, ChunkOptions};

#[test]
fn test_real_inbox_files() {
    // Read directory from env var or fallback to the requested path
    let inbox_path_str = std::env::var("SCRIBO_INBOX")
        .unwrap_or_else(|_| "/home/yoffens/obsidian2026/1-INBOX/".to_string());
    
    let inbox_path = PathBuf::from(&inbox_path_str);
    
    if !inbox_path.exists() || !inbox_path.is_dir() {
        println!(
            "Skipping real inbox files test: directory '{}' does not exist or is not a directory.",
            inbox_path_str
        );
        return;
    }

    println!("Scanning real inbox files at: {}", inbox_path.display());
    
    let entries = match std::fs::read_dir(&inbox_path) {
        Ok(e) => e,
        Err(err) => {
            println!("Failed to read inbox directory: {}. Skipping.", err);
            return;
        }
    };

    let mut processed_files = 0;
    let opts = ChunkOptions::default();

    for entry_res in entries {
        if let Ok(entry) = entry_res {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(err) => {
                        println!("Could not read note file '{}': {}", filename, err);
                        continue;
                    }
                };

                println!("Verifying chunking for real note: {}", filename);
                
                // Ensure no panic when chunking real Obsidian notes
                let result = chunk_paired(content, &opts);
                
                // Simple validation: if note is non-empty, we should get some chunks
                if !path.metadata().map(|m| m.len() == 0).unwrap_or(false) {
                    assert!(
                        !result.pairs.is_empty(),
                        "Note '{}' is non-empty but produced 0 chunks.",
                        filename
                    );
                }
                
                processed_files += 1;
            }
        }
    }

    println!("Successfully verified {} real notes from Obsidian Inbox.", processed_files);
}
