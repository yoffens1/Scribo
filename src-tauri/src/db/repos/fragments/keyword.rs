use rusqlite::Connection;
use crate::error::AppError;
use crate::domain::fragment::FragmentId;
use crate::domain::note::NoteId;
use crate::domain::search::{SearchHit, ScoredHit};

/// Escapes FTS5 operators, strips punctuation, removes common Russian and English stopwords,
/// and joins the remaining tokens with OR to prevent syntax errors and ensure robust matching.
pub fn clean_fts_query(query: &str) -> String {
    let lower = query.to_lowercase();
    let stopwords: std::collections::HashSet<&str> = [
        // English stopwords
        "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "any", "are", 
        "as", "at", "be", "because", "been", "before", "being", "below", "between", "both", "but", 
        "by", "did", "do", "does", "doing", "down", "during", "each", "few", "for", "from", "further", 
        "had", "has", "have", "having", "he", "her", "here", "hers", "herself", "him", "himself", 
        "his", "how", "i", "if", "in", "into", "is", "it", "its", "itself", "me", "more", "most", 
        "my", "myself", "no", "nor", "not", "of", "off", "on", "once", "only", "or", "other", "our", 
        "ours", "ourselves", "out", "over", "own", "same", "she", "should", "so", "some", "such", 
        "than", "that", "the", "their", "theirs", "them", "themselves", "then", "there", "these", 
        "they", "this", "those", "through", "to", "too", "under", "until", "up", "very", "was", 
        "we", "were", "what", "when", "where", "which", "while", "who", "whom", "why", "with", 
        "you", "your", "yours", "yourself", "yourselves",
        
        // Russian stopwords
        "и", "в", "во", "что", "такое", "как", "это", "не", "на", "с", "со", "он", "я", "у", 
        "то", "так", "для", "о", "об", "обо", "по", "из", "от", "до", "или", "бы", "ли", "же", 
        "чтобы", "если", "был", "была", "было", "были", "есть", "его", "ее", "их", "ему", "ей", 
        "ими", "ком", "чем", "а", "но", "да", "же", "уже", "или", "когда", "кто", "где", 
        "куда", "зачем", "почему", "кого", "кому", "кем"
    ].iter().cloned().collect();

    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();

    let filtered_words: Vec<String> = words
        .into_iter()
        .filter(|w| !stopwords.contains(w))
        .map(|w| format!("\"{}\"", w))
        .collect();

    if filtered_words.is_empty() {
        let fallback_words: Vec<String> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|w| format!("\"{}\"", w))
            .collect();
        if fallback_words.is_empty() {
            "".to_string()
        } else {
            fallback_words.join(" OR ")
        }
    } else {
        filtered_words.join(" OR ")
    }
}

/// FTS5/BM25 keyword search. Matches against the `chunks_fts` virtual table
/// and returns snippets with `<b>highlighted</b>` query terms.
/// Only searches `level = 1` chunks belonging to `'active'` notes.
pub fn search(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<ScoredHit>, AppError> {
    let clean_query = clean_fts_query(query);
    if clean_query.is_empty() {
        return Ok(Vec::new());
    }

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
    let rows = stmt.query_map(rusqlite::params![clean_query, limit], |row| {
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
