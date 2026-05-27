use std::path::PathBuf;
use rusqlite::Connection;

pub fn handle_import_dir(conn: &mut Connection, dir_path_str: &str) {
    let dir_path = PathBuf::from(dir_path_str);
    if !dir_path.is_dir() {
        println!("Error: Path is not a directory.");
        return;
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let embedder_config = crate::ai::types::EmbedderConfig {
            provider: "local".to_string(),
            model: Some("granite-embedding-97M-multilingual-r2-BF16".to_string()),
            api_key: None,
            base_url: None,
        };
        let embedder = crate::ai::embedding::Embedder::new(embedder_config);

        let mut md_files = Vec::new();
        for entry in walkdir::WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                md_files.push(path.to_path_buf());
            }
        }

        println!("Found {} markdown files to import.", md_files.len());

        let mut imported = 0;
        for path in md_files {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
            let note_id = match crate::services::import::import_markdown_file(conn, &path) {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Error importing file {:?}: {}", path, e);
                    continue;
                }
            };

            let payload = crate::services::indexer::IndexingPayload {
                note_id: note_id.0,
                embedding_model: "granite-embedding-97M-multilingual-r2-BF16",
                embedding_dim: 384,
                indexing_version: "1",
            };

            if let Err(e) = crate::services::indexer::persist_indexed_file(conn, payload) {
                eprintln!("Error indexing file {}: {}", file_name, e);
                continue;
            }

            let fragments = match crate::db::repos::fragments::list_by_note(conn, note_id.0) {
                Ok(frags) => frags,
                Err(e) => {
                    eprintln!("Error listing fragments for {}: {}", file_name, e);
                    continue;
                }
            };

            let mut fragment_embeddings = Vec::new();
            for frag in &fragments {
                match embedder.embed(&frag.text_clean).await {
                    Ok(emb) => {
                        fragment_embeddings.push((frag.fragment_index, emb));
                    }
                    Err(e) => {
                        eprintln!("Error embedding fragment for {}: {}", file_name, e);
                    }
                }
            }

            let mut error_occurred = false;
            for (index, emb) in fragment_embeddings {
                let emb_bytes = bytemuck::cast_slice::<f32, u8>(&emb);
                if let Err(e) = crate::db::repos::fragments::set_embedding(conn, note_id.0, index, emb_bytes) {
                    eprintln!("Error saving embedding for {}: {}", file_name, e);
                    error_occurred = true;
                    break;
                }
            }

            if !error_occurred {
                println!("Imported: {} (fragments: {})", file_name, fragments.len());
                imported += 1;
            }
        }

        println!("Successfully imported {} markdown notes with embeddings.", imported);
    });
}
