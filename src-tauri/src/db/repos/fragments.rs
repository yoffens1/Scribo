use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::fragment::{FragmentInsertRow, Fragment, FragmentId};
use crate::domain::note::NoteId;
use crate::domain::search::{SearchHit, ScoredHit};

#[derive(Debug, Clone)]
pub struct FragmentWithNote {
    pub fragment: Fragment,
    pub note_file_path: Option<String>,
    pub note_title: String,
}

pub fn delete_by_note_id(conn: &Connection, note_id: i64) -> Result<i64, AppError> {
    let deleted = conn.execute(
        "DELETE FROM fragments WHERE note_id = ?",
        rusqlite::params![note_id],
    )?;
    Ok(deleted as i64)
}

pub fn delete_by_id(conn: &Connection, id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM fragments WHERE fragment_id = ?",
        rusqlite::params![id],
    )?;
    Ok(())
}

pub fn insert(conn: &mut Connection, note_id: i64, rows: Vec<FragmentInsertRow>) -> Result<(), AppError> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO fragments (note_id, fragment_index, text_clean, source_hash, token_count, embedding)
             VALUES (?, ?, ?, ?, ?, ?)",
        )?;
        for row in &rows {
            stmt.execute(rusqlite::params![
                note_id,
                row.fragment_index,
                row.text_clean,
                row.source_hash,
                row.tokens,
                row.embedding
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn list_by_note(conn: &Connection, note_id: i64) -> Result<Vec<Fragment>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT fragment_id, note_id, fragment_index, text_clean, source_hash, token_count, embedding
         FROM fragments WHERE note_id = ? ORDER BY fragment_index ASC"
    )?;
    let rows = stmt.query_map([note_id], |row| {
        Ok(Fragment {
            id: FragmentId(row.get(0)?),
            note_id: NoteId(row.get(1)?),
            fragment_index: row.get(2)?,
            text_clean: row.get(3)?,
            source_hash: row.get(4)?,
            token_count: row.get(5)?,
            embedding: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn update(
    conn: &Connection,
    note_id: i64,
    index: i64,
    text_clean: &str,
    source_hash: &str,
    clear_embedding: bool,
) -> Result<(), AppError> {
    if clear_embedding {
        conn.execute(
            "UPDATE fragments 
             SET text_clean = ?, source_hash = ?, embedding = zeroblob(0) 
             WHERE note_id = ? AND fragment_index = ?",
            rusqlite::params![text_clean, source_hash, note_id, index],
        )?;
    } else {
        conn.execute(
            "UPDATE fragments 
             SET text_clean = ?, source_hash = ? 
             WHERE note_id = ? AND fragment_index = ?",
            rusqlite::params![text_clean, source_hash, note_id, index],
        )?;
    }
    Ok(())
}

pub fn insert_single(
    conn: &Connection,
    note_id: i64,
    index: i64,
    text_clean: &str,
    source_hash: &str,
    token_count: Option<i64>,
    embedding: &[u8],
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO fragments (note_id, fragment_index, text_clean, source_hash, token_count, embedding)
         VALUES (?, ?, ?, ?, ?, ?)",
        rusqlite::params![note_id, index, text_clean, source_hash, token_count, embedding],
    )?;
    Ok(())
}

pub fn set_embedding(
    conn: &Connection,
    note_id: i64,
    index: i64,
    embedding: &[u8],
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE fragments SET embedding = ? WHERE note_id = ? AND fragment_index = ?",
        rusqlite::params![embedding, note_id, index],
    )?;
    Ok(())
}

pub fn list_fragments_with_note(
    conn: &Connection,
    filter_note_id: Option<i64>,
    include_deleted: bool,
) -> Result<Vec<FragmentWithNote>, AppError> {
    let mut sql = "SELECT frag.fragment_id, NULL AS file_path, frag.fragment_index, frag.text_clean, frag.source_hash, frag.token_count, frag.embedding, frag.note_id, n.title
                   FROM fragments frag
                   JOIN notes n ON n.note_id = frag.note_id".to_string();

    let mut conditions = Vec::new();
    let mut params: Vec<&dyn rusqlite::types::ToSql> = Vec::new();

    if !include_deleted {
        conditions.push("n.is_deleted = 0");
    }
    if let Some(ref note_id) = filter_note_id {
        conditions.push("n.note_id = ?");
        params.push(note_id);
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    sql.push_str(" ORDER BY n.note_id, frag.fragment_index");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
        Ok(FragmentWithNote {
            fragment: Fragment {
                id: FragmentId(row.get(0)?),
                note_id: NoteId(row.get(7)?),
                fragment_index: row.get(2)?,
                text_clean: row.get(3)?,
                source_hash: row.get(4)?,
                token_count: row.get(5)?,
                embedding: row.get(6)?,
            },
            note_file_path: row.get(1)?,
            note_title: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn search(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<ScoredHit>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT frag.fragment_id,
                NULL AS file_path,
                frag.fragment_index,
                snippet(fragments_fts, 0, '<b>', '</b>', '…', 32),
                bm25(fragments_fts),
                n.title,
                n.note_id,
                frag.text_clean
         FROM fragments_fts
         JOIN fragments frag ON frag.fragment_id = fragments_fts.rowid
         JOIN notes n ON n.note_id = frag.note_id
         WHERE fragments_fts MATCH ?
           AND n.is_deleted = 0
         ORDER BY bm25(fragments_fts)
         LIMIT ?",
    )?;
    let rows = stmt.query_map(rusqlite::params![query, limit], |row| {
        let fragment_id = FragmentId(row.get(0)?);
        let note_id = NoteId(row.get(6)?);
        let score = row.get::<_, f64>(4)? as f32;
        Ok(ScoredHit {
            hit: SearchHit {
                fragment_id,
                note_id,
                fragment_index: row.get(2)?,
                text: row.get(7)?,
                note_title: row.get(5)?,
                note_file_path: row.get(1)?,
                snippet: Some(row.get(3)?),
            },
            score,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

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

#[derive(Debug)]
struct HitRecord {
    fragment_id: i64,
    similarity: f32,
}
impl PartialEq for HitRecord {
    fn eq(&self, other: &Self) -> bool {
        self.fragment_id == other.fragment_id
    }
}
impl Eq for HitRecord {}
impl PartialOrd for HitRecord {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        other.similarity.partial_cmp(&self.similarity)
    }
}
impl Ord for HitRecord {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

pub fn vector_search(
    conn: &Connection,
    query_embedding_bytes: &[u8],
    limit: usize,
) -> Result<Vec<ScoredHit>, AppError> {
    let query_vector = bytes_to_f32_slice(query_embedding_bytes);

    let mut top_hits = std::collections::BinaryHeap::with_capacity(limit + 1);

    {
        let mut stmt = conn.prepare(
            "SELECT frag.fragment_id, frag.embedding
             FROM fragments frag
             JOIN notes n ON n.note_id = frag.note_id
             WHERE n.is_deleted = 0",
        )?;

        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let fragment_id: i64 = row.get(0)?;
            let blob_ref = row.get_ref(1)?;
            if let rusqlite::types::ValueRef::Blob(bytes) = blob_ref {
                let cand_vector = bytes_to_f32_slice(bytes);
                let similarity = cosine_similarity(query_vector, cand_vector);
                
                top_hits.push(HitRecord { fragment_id, similarity });
                if top_hits.len() > limit {
                    top_hits.pop();
                }
            }
        }
    }

    let mut hits: Vec<HitRecord> = top_hits.into_sorted_vec();
    hits.reverse();

    if hits.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<String> = hits.iter().map(|h| h.fragment_id.to_string()).collect();
    let in_clause = ids.join(",");

    let sql = format!(
        "SELECT frag.fragment_id, NULL AS file_path, frag.fragment_index, frag.text_clean, n.title, n.note_id
         FROM fragments frag
         JOIN notes n ON n.note_id = frag.note_id
         WHERE frag.fragment_id IN ({})",
         in_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let row_iter = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;

    let mut db_data = std::collections::HashMap::new();
    for row in row_iter {
        let (id, path, idx, text, title, note_id_val) = row?;
        db_data.insert(id, (path, idx, text, title, note_id_val));
    }

    let mut final_results = Vec::with_capacity(hits.len());
    for h in hits {
        if let Some((path, idx, text, title, note_id_val)) = db_data.remove(&h.fragment_id) {
            final_results.push(ScoredHit {
                hit: SearchHit {
                    fragment_id: FragmentId(h.fragment_id),
                    note_id: NoteId(note_id_val),
                    fragment_index: idx,
                    text,
                    note_title: Some(title),
                    note_file_path: path,
                    snippet: None,
                },
                score: h.similarity,
            });
        }
    }

    Ok(final_results)
}
