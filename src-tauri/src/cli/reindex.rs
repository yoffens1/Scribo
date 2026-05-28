//! # CLI Reindex Handler
//!
//! Subcommand to re-calculate and update embeddings for all fragments in the database.
//! Useful after changing the embedding context length or model.

use rusqlite::Connection;
use crate::ai::embedding::Embedder;
use crate::ai::types::EmbedderConfig;

pub fn handle_reindex(conn: &mut Connection) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let embedder_config = EmbedderConfig {
            provider: "local".to_string(),
            model: Some("granite-embedding-97M-multilingual-r2-BF16".to_string()),
            api_key: None,
            base_url: None,
        };
        let embedder = Embedder::new(embedder_config);

        println!("Fetching all fragments from the database...");
        
        // List all notes
        let notes = match crate::db::repos::notes::get_all(conn) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("Error fetching notes: {}", e);
                return;
            }
        };

        let total_notes = notes.len();
        println!("Found {} notes to reindex.", total_notes);

        for (note_idx, note) in notes.into_iter().enumerate() {
            let note_id = note.note_id.0;
            println!(
                "[{}/{}] Reindexing note '{}' (ID: {})...",
                note_idx + 1,
                total_notes,
                note.title,
                note_id
            );

            let fragments = match crate::db::repos::fragments::list_by_note(conn, note_id) {
                Ok(frags) => frags,
                Err(e) => {
                    eprintln!("  Error listing fragments: {}", e);
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
                        eprintln!("  Error embedding fragment {}: {}", frag.fragment_index, e);
                    }
                }
            }

            let mut success_count = 0;
            for (index, emb) in fragment_embeddings {
                let emb_bytes = bytemuck::cast_slice::<f32, u8>(&emb);
                if let Err(e) = crate::db::repos::fragments::set_embedding(conn, note_id, index, emb_bytes) {
                    eprintln!("  Error saving embedding for fragment {}: {}", index, e);
                } else {
                    success_count += 1;
                }
            }

            println!(
                "  Completed: {}/{} fragments updated.",
                success_count,
                fragments.len()
            );
        }

        println!("Reindexing complete!");
    });
}
