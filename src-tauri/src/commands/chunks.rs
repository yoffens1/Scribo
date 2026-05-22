use crate::error::AppError;
use crate::DbState;
use serde::{Deserialize, Serialize};
use tauri::State;

// ── Write serialization ───────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkInsertRow {
    pub chunk_index: i64,
    pub text: String,
    pub tokens: i64,
    #[serde(with = "serde_bytes")]
    pub embedding: Vec<u8>,
}

#[tauri::command]
pub fn chunks_delete_by_file_id(
    state: State<'_, DbState>,
    file_id: i64,
) -> Result<i64, AppError> {
    let _w = state.write_lock.lock();
    state.with_conn(|conn| {
        // execute() returns affected rows — no need for a prior COUNT query.
        let deleted = conn.execute(
            "DELETE FROM chunks WHERE file_id = ?",
            rusqlite::params![file_id],
        )?;
        Ok(deleted as i64)
    })
}

#[tauri::command]
pub fn chunks_insert(
    state: State<'_, DbState>,
    file_id: i64,
    rows: Vec<ChunkInsertRow>,
) -> Result<(), AppError> {
    let _w = state.write_lock.lock();
    state.with_conn(|conn| {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO chunks (file_id, chunk_index, chunk_text, token_count, embedding)
                 VALUES (?, ?, ?, ?, ?)",
            )?;
            for row in &rows {
                stmt.execute(rusqlite::params![
                    file_id,
                    row.chunk_index,
                    row.text,
                    row.tokens,
                    row.embedding
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
}

// ── Shared record types ───────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullChunkRecord {
    pub chunk_id: i64,
    pub file_path: String,
    pub chunk_index: i64,
    pub chunk_text: Option<String>,
    pub token_count: Option<i64>,
    #[serde(with = "serde_bytes")]
    pub embedding: Vec<u8>,
}

// ── Private query helper ──────────────────────────────────────────────────────
//
// All three public "get chunks" commands delegate here. The caller provides
// an optional WHERE clause fragment (already joined with AND) and the params.
// This eliminates ~100 lines of near-identical boilerplate.

fn fetch_chunks(
    conn: &mut rusqlite::Connection,
    extra_where: &str,
    params: &[&dyn rusqlite::types::ToSql],
) -> Result<Vec<FullChunkRecord>, AppError> {
    let base = "SELECT c.chunk_id, f.file_path, c.chunk_index, c.chunk_text, c.token_count, c.embedding
                FROM chunks c
                JOIN files f ON f.file_id = c.file_id";
    let sql = if extra_where.is_empty() {
        format!("{} ORDER BY f.file_path, c.chunk_index", base)
    } else {
        format!("{} WHERE {} ORDER BY f.file_path, c.chunk_index", base, extra_where)
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params, |row| {
        Ok(FullChunkRecord {
            chunk_id: row.get(0)?,
            file_path: row.get(1)?,
            chunk_index: row.get(2)?,
            chunk_text: row.get(3)?,
            token_count: row.get(4)?,
            embedding: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

// ── Public query commands ─────────────────────────────────────────────────────

#[tauri::command]
pub fn chunks_get_by_file_path(
    state: State<'_, DbState>,
    file_path: String,
    include_deleted: bool,
) -> Result<Vec<FullChunkRecord>, AppError> {
    state.with_conn(|conn| {
        let clause = if include_deleted {
            "f.file_path = ?"
        } else {
            "f.file_path = ? AND f.is_deleted = 0"
        };
        fetch_chunks(conn, clause, &[&file_path])
    })
}

#[tauri::command]
pub fn chunks_get_all(
    state: State<'_, DbState>,
    include_deleted: bool,
) -> Result<Vec<FullChunkRecord>, AppError> {
    state.with_conn(|conn| {
        let clause = if include_deleted { "" } else { "f.is_deleted = 0" };
        fetch_chunks(conn, clause, &[])
    })
}

#[tauri::command]
pub fn chunks_get_by_file_name(
    state: State<'_, DbState>,
    name: String,
    include_deleted: bool,
) -> Result<Vec<FullChunkRecord>, AppError> {
    state.with_conn(|conn| {
        // Name normalization (.md suffix) is the caller's responsibility.
        let clause = if include_deleted {
            "f.file_name = ?"
        } else {
            "f.file_name = ? AND f.is_deleted = 0"
        };
        fetch_chunks(conn, clause, &[&name])
    })
}

// ── FTS5 full-text search ─────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub chunk_id: i64,
    pub file_path: String,
    pub chunk_index: i64,
    /// HTML snippet with `<b>...</b>` highlights around matched terms.
    pub snippet: String,
    /// BM25 score — lower (more negative) is better.
    pub score: f64,
}

#[tauri::command]
pub fn chunks_search(
    state: State<'_, DbState>,
    query: String,
    limit: i64,
) -> Result<Vec<SearchHit>, AppError> {
    state.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT c.chunk_id,
                    f.file_path,
                    c.chunk_index,
                    snippet(chunks_fts, 0, '<b>', '</b>', '…', 32),
                    bm25(chunks_fts)
             FROM chunks_fts
             JOIN chunks c ON c.chunk_id = chunks_fts.rowid
             JOIN files  f ON f.file_id  = c.file_id
             WHERE chunks_fts MATCH ?
               AND f.is_deleted = 0
             ORDER BY bm25(chunks_fts)
             LIMIT ?",
        )?;
        let rows = stmt.query_map(rusqlite::params![query, limit], |row| {
            Ok(SearchHit {
                chunk_id: row.get(0)?,
                file_path: row.get(1)?,
                chunk_index: row.get(2)?,
                snippet: row.get(3)?,
                score: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    })
}

// ── Vector semantic search ──────────────────────────────────────────────────

fn bytes_to_f32_slice(bytes: &[u8]) -> &[f32] {
    bytemuck::cast_slice(bytes)
}

fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    let mut dot_product = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    
    for (a, b) in v1.iter().zip(v2.iter()) {
        dot_product += a * b;
        norm_a += a * a;
        norm_b += b * b;
    }
    
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a.sqrt() * norm_b.sqrt())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorSearchHit {
    pub chunk_id: i64,
    pub file_path: String,
    pub chunk_index: i64,
    pub chunk_text: Option<String>,
    pub similarity: f32,
}

#[tauri::command]
pub async fn chunks_vector_search(
    state: State<'_, DbState>,
    query_embedding_bytes: Vec<u8>,
    limit: usize,
) -> Result<Vec<VectorSearchHit>, AppError> {
    let pool = state.inner().pool.read().as_ref().cloned().ok_or(AppError::NotInitialized)?;

    tauri::async_runtime::spawn_blocking(move || {
        let query_vector = bytes_to_f32_slice(&query_embedding_bytes);
        let conn = pool.get().map_err(|e| AppError::Other(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT c.chunk_id, f.file_path, c.chunk_index, c.chunk_text, c.embedding
             FROM chunks c
             JOIN files f ON f.file_id = c.file_id
             WHERE f.is_deleted = 0",
        )?;

        let mut hits = stmt.query_map([], |row| {
            let chunk_id: i64 = row.get(0)?;
            let file_path: String = row.get(1)?;
            let chunk_index: i64 = row.get(2)?;
            let chunk_text: Option<String> = row.get(3)?;
            // SQLite returns the BLOB as a Vec<u8> natively. 
            // We get ownership of it, then borrow it to get a slice.
            let blob_bytes: Vec<u8> = row.get(4)?;
            
            let cand_vector = bytes_to_f32_slice(&blob_bytes);
            let similarity = cosine_similarity(query_vector, cand_vector);

            Ok(VectorSearchHit {
                chunk_id,
                file_path,
                chunk_index,
                chunk_text,
                similarity,
            })
        })?
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;

        hits.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(limit);

        Ok(hits)
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))?
}
