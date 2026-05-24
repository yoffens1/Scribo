use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::fragment::{Fragment, FragmentInsertRow, SearchHit, VectorSearchHit};



pub fn delete_by_note_id(conn: &Connection, note_id: i64) -> Result<i64, AppError> {
    let deleted = conn.execute(
        "DELETE FROM fragments WHERE note_id = ?",
        rusqlite::params![note_id],
    )?;
    Ok(deleted as i64)
}

pub fn insert(conn: &mut Connection, note_id: i64, rows: Vec<FragmentInsertRow>) -> Result<(), AppError> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO fragments (note_id, fragment_index, text, token_count, embedding)
             VALUES (?, ?, ?, ?, ?)",
        )?;
        for row in &rows {
            stmt.execute(rusqlite::params![
                note_id,
                row.fragment_index,
                row.text,
                row.tokens,
                row.embedding
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}



fn fetch_fragments(
    conn: &Connection,
    extra_where: &str,
    params: &[&dyn rusqlite::types::ToSql],
) -> Result<Vec<Fragment>, AppError> {
    let base = "SELECT c.fragment_id, f.file_path, c.fragment_index, c.text, c.token_count, c.embedding
                FROM fragments c
                JOIN notes f ON f.note_id = c.note_id";
    let sql = if extra_where.is_empty() {
        format!("{} ORDER BY f.file_path, c.fragment_index", base)
    } else {
        format!("{} WHERE {} ORDER BY f.file_path, c.fragment_index", base, extra_where)
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params, |row| {
        Ok(Fragment {
            id: crate::domain::fragment::FragmentId(row.get(0)?),
            note_id: crate::domain::note::NoteId(0), // Dummy value since query doesn't fetch it
            text: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            fragment_id: row.get(0)?,
            file_path: row.get(1)?,
            fragment_index: row.get(2)?,
            fragment_text: row.get(3)?,
            token_count: row.get(4)?,
            embedding: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get_by_file_path(
    conn: &Connection,
    file_path: &str,
    include_deleted: bool,
) -> Result<Vec<Fragment>, AppError> {
    let clause = if include_deleted {
        "f.file_path = ?"
    } else {
        "f.file_path = ? AND f.is_deleted = 0"
    };
    fetch_fragments(conn, clause, &[&file_path])
}

pub fn get_all(
    conn: &Connection,
    include_deleted: bool,
) -> Result<Vec<Fragment>, AppError> {
    let clause = if include_deleted { "" } else { "f.is_deleted = 0" };
    fetch_fragments(conn, clause, &[])
}

pub fn get_by_file_name(
    conn: &Connection,
    name: &str,
    include_deleted: bool,
) -> Result<Vec<Fragment>, AppError> {
    let clause = if include_deleted {
        "f.file_name = ?"
    } else {
        "f.file_name = ? AND f.is_deleted = 0"
    };
    fetch_fragments(conn, clause, &[&name])
}



pub fn search(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<SearchHit>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT c.fragment_id,
                f.file_path,
                c.fragment_index,
                snippet(fragments_fts, 0, '<b>', '</b>', '…', 32),
                bm25(fragments_fts)
         FROM fragments_fts
         JOIN fragments c ON c.fragment_id = fragments_fts.rowid
         JOIN notes  f ON f.note_id  = c.note_id
         WHERE fragments_fts MATCH ?
           AND f.is_deleted = 0
         ORDER BY bm25(fragments_fts)
         LIMIT ?",
    )?;
    let rows = stmt.query_map(rusqlite::params![query, limit], |row| {
        Ok(SearchHit {
            fragment_id: row.get(0)?,
            file_path: row.get(1)?,
            fragment_index: row.get(2)?,
            snippet: row.get(3)?,
            score: row.get(4)?,
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
) -> Result<Vec<VectorSearchHit>, AppError> {
    let query_vector = bytes_to_f32_slice(query_embedding_bytes);

    let mut top_hits = std::collections::BinaryHeap::with_capacity(limit + 1);

    {
        let mut stmt = conn.prepare(
            "SELECT c.fragment_id, c.embedding
             FROM fragments c
             JOIN notes f ON f.note_id = c.note_id
             WHERE f.is_deleted = 0",
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
        "SELECT c.fragment_id, f.file_path, c.fragment_index, c.text
         FROM fragments c
         JOIN notes f ON f.note_id = c.note_id
         WHERE c.fragment_id IN ({})",
        in_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let row_iter = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    let mut db_data = std::collections::HashMap::new();
    for row in row_iter {
        let (id, path, idx, text) = row?;
        db_data.insert(id, (path, idx, text));
    }

    let mut final_results = Vec::with_capacity(hits.len());
    for h in hits {
        if let Some((path, idx, text)) = db_data.remove(&h.fragment_id) {
            final_results.push(VectorSearchHit {
                fragment_id: h.fragment_id,
                file_path: path,
                fragment_index: idx,
                fragment_text: text,
                similarity: h.similarity,
            });
        }
    }

    Ok(final_results)
}
