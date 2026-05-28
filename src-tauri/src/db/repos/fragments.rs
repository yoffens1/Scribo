use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::fragment::{FragmentInsertRow, Fragment, FragmentId};
use crate::domain::note::NoteId;
use crate::domain::search::{SearchHit, ScoredHit};

#[derive(Debug, Clone)]
pub struct FragmentWithNote {
    pub fragment: Fragment,
    pub note_path: Option<String>,
    pub note_title: String,
}

pub fn delete_by_note_id(conn: &Connection, note_id: i64) -> Result<i64, AppError> {
    let deleted = conn.execute(
        "DELETE FROM chunks WHERE note_id = ? AND level = 1",
        rusqlite::params![note_id],
    )?;
    Ok(deleted as i64)
}

pub fn delete_by_id(conn: &Connection, id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM chunks WHERE chunk_id = ? AND level = 1",
        rusqlite::params![id],
    )?;
    Ok(())
}

pub fn insert(conn: &mut Connection, note_id: i64, rows: Vec<FragmentInsertRow>) -> Result<(), AppError> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, token_count, embedding, kind)
             VALUES (?, 1, ?, ?, ?, ?, ?, ?, ?, 'fragment')",
        )?;
        for row in &rows {
            stmt.execute(rusqlite::params![
                note_id,
                row.fragment_index,
                row.text_clean,
                row.clean_hash,
                row.text_clean,
                row.clean_hash,
                row.token_count,
                row.embedding
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn list_by_note(conn: &Connection, note_id: i64) -> Result<Vec<Fragment>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT chunk_id, note_id, order_index, clean_text, clean_text_hash, token_count, embedding, parent_chunk_id
         FROM chunks WHERE note_id = ? AND level = 1 ORDER BY order_index ASC"
    )?;
    let rows = stmt.query_map([note_id], |row| {
        let parent_chunk_id: Option<i64> = row.get(7)?;
        Ok(Fragment {
            id: FragmentId(row.get(0)?),
            note_id: NoteId(row.get(1)?),
            section_id: parent_chunk_id.map(crate::domain::SectionId),
            fragment_index: row.get(2)?,
            text_clean: row.get(3)?,
            clean_hash: row.get(4)?,
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
    clean_hash: &str,
    clear_embedding: bool,
    parent_chunk_id: Option<i64>,
) -> Result<(), AppError> {
    if clear_embedding {
        conn.execute(
            "UPDATE chunks 
             SET raw_text = ?, raw_text_hash = ?, clean_text = ?, clean_text_hash = ?, embedding = NULL, parent_chunk_id = ? 
             WHERE note_id = ? AND order_index = ? AND level = 1",
            rusqlite::params![text_clean, clean_hash, text_clean, clean_hash, parent_chunk_id, note_id, index],
        )?;
    } else {
        conn.execute(
            "UPDATE chunks 
             SET raw_text = ?, raw_text_hash = ?, clean_text = ?, clean_text_hash = ?, parent_chunk_id = ? 
             WHERE note_id = ? AND order_index = ? AND level = 1",
            rusqlite::params![text_clean, clean_hash, text_clean, clean_hash, parent_chunk_id, note_id, index],
        )?;
    }
    Ok(())
}

pub fn insert_single(
    conn: &Connection,
    note_id: i64,
    index: i64,
    text_clean: &str,
    clean_hash: &str,
    token_count: Option<i64>,
    embedding: &[u8],
    parent_chunk_id: Option<i64>,
) -> Result<i64, AppError> {
    conn.execute(
        "INSERT INTO chunks (note_id, level, order_index, raw_text, raw_text_hash, clean_text, clean_text_hash, token_count, embedding, parent_chunk_id, kind)
         VALUES (?, 1, ?, ?, ?, ?, ?, ?, ?, ?, 'fragment')",
        rusqlite::params![note_id, index, text_clean, clean_hash, text_clean, clean_hash, token_count, embedding, parent_chunk_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn set_embedding(
    conn: &Connection,
    note_id: i64,
    index: i64,
    embedding: &[u8],
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE chunks SET embedding = ? WHERE note_id = ? AND order_index = ? AND level = 1",
        rusqlite::params![embedding, note_id, index],
    )?;
    Ok(())
}

pub fn list_fragments_with_note(
    conn: &Connection,
    filter_note_id: Option<i64>,
    include_deleted: bool,
) -> Result<Vec<FragmentWithNote>, AppError> {
    let mut sql = "SELECT frag.chunk_id, n.path_cached, frag.order_index, frag.clean_text, frag.clean_text_hash, frag.token_count, frag.embedding, frag.note_id, n.title, frag.parent_chunk_id
                   FROM chunks frag
                   JOIN notes n ON n.note_id = frag.note_id
                   WHERE frag.level = 1".to_string();

    let mut conditions = Vec::new();
    let mut params: Vec<&dyn rusqlite::types::ToSql> = Vec::new();

    if !include_deleted {
        conditions.push("n.lifecycle != 'deleted'");
    }
    if let Some(ref note_id) = filter_note_id {
        conditions.push("n.note_id = ?");
        params.push(note_id);
    }

    if !conditions.is_empty() {
        sql.push_str(" AND ");
        sql.push_str(&conditions.join(" AND "));
    }

    sql.push_str(" ORDER BY n.note_id, frag.order_index");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
        let parent_chunk_id: Option<i64> = row.get(9)?;
        Ok(FragmentWithNote {
            fragment: Fragment {
                id: FragmentId(row.get(0)?),
                note_id: NoteId(row.get(7)?),
                section_id: parent_chunk_id.map(crate::domain::SectionId),
                fragment_index: row.get(2)?,
                text_clean: row.get(3)?,
                clean_hash: row.get(4)?,
                token_count: row.get(5)?,
                embedding: row.get(6)?,
            },
            note_path: row.get(1)?,
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
        "SELECT frag.chunk_id,
                n.path_cached,
                frag.order_index,
                snippet(chunks_fts, 0, '<b>', '</b>', '…', 32),
                bm25(chunks_fts),
                n.title,
                n.note_id,
                frag.clean_text
         FROM chunks_fts
         JOIN chunks frag ON frag.chunk_id = chunks_fts.rowid
         JOIN notes n ON n.note_id = frag.note_id
         WHERE chunks_fts MATCH ?
           AND frag.level = 1
           AND n.lifecycle = 'active'
         ORDER BY bm25(chunks_fts)
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
                note_title: Some(row.get(5)?),
                note_path: row.get(1)?,
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

use crate::utils::cosine_similarity;

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
    level: Option<i64>,
    limit: usize,
) -> Result<Vec<ScoredHit>, AppError> {
    let query_vector = bytes_to_f32_slice(query_embedding_bytes);

    let mut top_hits = std::collections::BinaryHeap::with_capacity(limit + 1);

    {
        let sql = if let Some(l) = level {
            format!(
                "SELECT frag.chunk_id, frag.embedding
                 FROM chunks frag
                 JOIN notes n ON n.note_id = frag.note_id
                 WHERE frag.level = {} AND n.lifecycle = 'active'",
                l
            )
        } else {
            "SELECT frag.chunk_id, frag.embedding
             FROM chunks frag
             JOIN notes n ON n.note_id = frag.note_id
             WHERE n.lifecycle = 'active'".to_string()
        };
        let mut stmt = conn.prepare(&sql)?;

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
        "SELECT frag.chunk_id, n.path_cached, frag.order_index, frag.clean_text, n.title, n.note_id
         FROM chunks frag
         JOIN notes n ON n.note_id = frag.note_id
         WHERE frag.chunk_id IN ({})",
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
                    note_path: path,
                    snippet: None,
                },
                score: h.similarity,
            });
        }
    }

    Ok(final_results)
}
