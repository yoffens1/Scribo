use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::fragment::FragmentId;
use crate::domain::note::NoteId;
use crate::domain::search::{SearchHit, ScoredHit};


/// Tokenizes the raw query, removes tokens shorter than 3 characters (not useful for trigrams),
/// and returns clean tokens (no quotes) for FTS5.
pub fn get_fts_query_tokens(query: &str) -> Vec<String> {
    let lower = query.to_lowercase();

    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();

    let mut filtered_words: Vec<String> = words
        .into_iter()
        .filter(|w| w.len() >= 3)
        .map(|w| w.to_string())
        .collect();

    if filtered_words.is_empty() {
        filtered_words = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|w| w.to_string())
            .collect();
    }
    filtered_words
}

/// Escapes FTS5 operators, strips punctuation, removes common Russian and English stopwords,
/// and joins the remaining tokens with OR. Kept for backwards compatibility.
pub fn clean_fts_query(query: &str) -> String {
    let tokens = get_fts_query_tokens(query);
    tokens.join(" OR ")
}

/// FTS5/BM25 keyword search. Matches against the `chunks_fts` virtual table
/// and returns snippets with `<b>highlighted</b>` query terms.
/// Only searches `level = 1` chunks belonging to `'active'` notes.
/// Tries AND-matching first, and falls back to OR-matching if no results are found.
pub fn search(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<ScoredHit>, AppError> {
    let tokens = get_fts_query_tokens(query);
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    // 1. Try search with AND
    let and_query = tokens.join(" AND ");
    let mut results = execute_fts_query(conn, &and_query, limit)?;

    // 2. Fall back to OR if AND returned nothing and we have multiple tokens
    if results.is_empty() && tokens.len() > 1 {
        let or_query = tokens.join(" OR ");
        results = execute_fts_query(conn, &or_query, limit)?;
    }

    Ok(results)
}

fn execute_fts_query(
    conn: &Connection,
    match_str: &str,
    limit: i64,
) -> Result<Vec<ScoredHit>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT frag.fragment_id,
                 n.path_cached,
                 frag.order_index,
                 snippet(fragments_fts, 0, '<b>', '</b>', '…', 32),
                 bm25(fragments_fts),
                 n.title,
                 n.note_id,
                 frag.clean_text
          FROM fragments_fts
          JOIN fragments frag ON frag.fragment_id = fragments_fts.rowid
          JOIN notes n ON n.note_id = frag.note_id
          WHERE fragments_fts MATCH ?
            AND frag.level = 1
            AND n.lifecycle = 'active'
          ORDER BY bm25(fragments_fts)
          LIMIT ?",
    )?;
    let rows = stmt.query_map(rusqlite::params![match_str, limit], |row| {
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
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}
