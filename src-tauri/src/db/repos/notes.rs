use rusqlite::{Connection, OptionalExtension};
use crate::error::AppError;
use crate::domain::note::{Note, NoteId, IndexingStatus};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NoteListItem {
    pub note_id: NoteId,
    pub title: String,
    pub is_deleted: bool,
    pub embedding_model: Option<String>,
    pub indexing_version: Option<String>,
}

fn row_to_note(row: &rusqlite::Row) -> rusqlite::Result<Note> {
    let parent_note_id: Option<i64> = row.get(4)?;
    let status_str: String = row.get(8)?;
    Ok(Note {
        id: NoteId(row.get(0)?),
        title: row.get(1)?,
        content: row.get(2)?,
        content_hash: row.get(3)?,
        parent_note_id: parent_note_id.map(NoteId),
        path_cached: row.get(5)?,
        sort_order: row.get(6)?,
        icon: row.get(7)?,
        indexing_status: IndexingStatus::parse(&status_str).unwrap_or(IndexingStatus::Pending),
        indexing_error: row.get(9)?,
        indexed_at: row.get(10)?,
        embedding_model: row.get(11)?,
        embedding_dimension: row.get(12)?,
        indexing_version: row.get(13)?,
        is_draft: row.get::<_, i64>(14).unwrap_or(0) != 0,
        is_archived: row.get::<_, i64>(15).unwrap_or(0) != 0,
        is_deleted: row.get::<_, i64>(16).unwrap_or(0) != 0,
        is_pinned: row.get::<_, i64>(17).unwrap_or(0) != 0,
        is_favorite: row.get::<_, i64>(18).unwrap_or(0) != 0,
        mastery: row.get(19)?,
        last_studied: row.get(20)?,
        created_at: row.get(21)?,
        updated_at: row.get(22)?,
    })
}

const SELECT_NOTE_COLUMNS: &str = 
    "SELECT note_id, title, content, content_hash, 
            parent_note_id, path_cached, sort_order, icon,
            indexing_status, indexing_error, indexed_at, embedding_model, embedding_dimension, 
            indexing_version, is_draft, is_archived, is_deleted, is_pinned, is_favorite,
            mastery, last_studied, created_at, updated_at
     FROM notes";

pub fn get_by_id(conn: &Connection, note_id: i64) -> Result<Option<Note>, AppError> {
    let sql = format!("{} WHERE note_id = ?", SELECT_NOTE_COLUMNS);
    let mut stmt = conn.prepare(&sql)?;
    let record = stmt.query_row([note_id], row_to_note).optional()?;
    Ok(record)
}

fn get_path_for_note(conn: &Connection, parent_id: Option<NoteId>, title: &str) -> Result<String, AppError> {
    if let Some(pid) = parent_id {
        let parent_path: Option<String> = conn.query_row(
            "SELECT path_cached FROM notes WHERE note_id = ?",
            [pid.0],
            |r| r.get(0)
        ).optional()?;
        if let Some(p_path) = parent_path {
            return Ok(format!("{}/{}", p_path, title));
        }
    }
    Ok(title.to_string())
}

fn recalculate_descendant_paths(conn: &Connection, parent_id: NoteId, parent_path: &str) -> Result<(), AppError> {
    let mut stmt = conn.prepare("SELECT note_id, title FROM notes WHERE parent_note_id = ? AND is_deleted = 0")?;
    let mut rows = stmt.query([parent_id.0])?;
    let mut children = Vec::new();
    while let Some(row) = rows.next()? {
        let child_id: i64 = row.get(0)?;
        let child_title: String = row.get(1)?;
        children.push((child_id, child_title));
    }
    for (child_id, child_title) in children {
        let child_path = format!("{}/{}", parent_path, child_title);
        conn.execute(
            "UPDATE notes SET path_cached = ? WHERE note_id = ?",
            rusqlite::params![child_path, child_id],
        )?;
        recalculate_descendant_paths(conn, NoteId(child_id), &child_path)?;
    }
    Ok(())
}

pub fn insert(conn: &Connection, note: &crate::domain::note::NewNote) -> Result<NoteId, AppError> {
    let now = crate::db::time::now_seconds();
    let content_hash = blake3::hash(note.content.as_bytes()).to_hex().to_string();

    let path_cached = match &note.path_cached {
        Some(p) => p.clone(),
        None => get_path_for_note(conn, note.parent_note_id, &note.title)?,
    };

    let parent_id = note.parent_note_id.map(|id| id.0);
    let sort_order = note.sort_order.unwrap_or(0);
    let is_draft_int = if note.is_draft { 1 } else { 0 };
    let is_pinned_int = if note.is_pinned { 1 } else { 0 };
    let is_favorite_int = if note.is_favorite { 1 } else { 0 };

    let note_id: i64 = conn.query_row(
        "INSERT INTO notes (
            title, content, content_hash, parent_note_id, path_cached, sort_order, icon,
            indexing_status, is_draft, is_archived, is_deleted, is_pinned, is_favorite, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?, 0, 0, ?, ?, ?, ?)
         RETURNING note_id",
        rusqlite::params![
            note.title,
            note.content,
            content_hash,
            parent_id,
            path_cached,
            sort_order,
            note.icon,
            is_draft_int,
            is_pinned_int,
            is_favorite_int,
            now,
            now,
        ],
        |row| row.get(0),
    )?;
    Ok(NoteId(note_id))
}

pub fn mark_indexed(conn: &Connection, note_id: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET indexing_status = 'indexed', indexing_error = NULL, indexed_at = ? WHERE note_id = ?",
        rusqlite::params![crate::db::time::now_seconds(), note_id],
    )?;
    Ok(())
}

pub fn record_failure(conn: &Connection, note_id: i64, error: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET indexing_status = 'failed', indexing_error = ?, updated_at = ? WHERE note_id = ?",
        rusqlite::params![error, crate::db::time::now_seconds(), note_id],
    )?;
    Ok(())
}

pub fn soft_delete(conn: &Connection, note_id: i64, updated_at: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET is_deleted = 1, updated_at = ? WHERE note_id = ?",
        rusqlite::params![updated_at, note_id],
    )?;
    Ok(())
}

pub fn restore(conn: &Connection, note_id: i64, updated_at: i64) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET is_deleted = 0, updated_at = ? WHERE note_id = ?",
        rusqlite::params![updated_at, note_id],
    )?;
    Ok(())
}

pub fn rename(conn: &Connection, note_id: i64, new_title: &str, updated_at: i64) -> Result<(), AppError> {
    let parent_id_opt: Option<i64> = conn.query_row(
        "SELECT parent_note_id FROM notes WHERE note_id = ?",
        [note_id],
        |r| r.get(0)
    ).optional()?.flatten();

    let new_path = get_path_for_note(conn, parent_id_opt.map(NoteId), new_title)?;

    conn.execute(
        "UPDATE notes SET title = ?, path_cached = ?, updated_at = ? WHERE note_id = ?",
        rusqlite::params![new_title, new_path, updated_at, note_id],
    )?;

    recalculate_descendant_paths(conn, NoteId(note_id), &new_path)?;
    Ok(())
}

fn is_descendant(conn: &Connection, ancestor_id: i64, descendant_id: i64) -> Result<bool, AppError> {
    let mut current_id = descendant_id;
    loop {
        let parent_id_opt: Option<i64> = conn.query_row(
            "SELECT parent_note_id FROM notes WHERE note_id = ?",
            [current_id],
            |r| r.get(0)
        ).optional()?;
        
        match parent_id_opt {
            Some(pid) => {
                if pid == ancestor_id {
                    return Ok(true);
                }
                current_id = pid;
            }
            None => break,
        }
    }
    Ok(false)
}

pub fn move_note(conn: &Connection, note_id: i64, new_parent_id: Option<NoteId>, updated_at: i64) -> Result<(), AppError> {
    if let Some(pid) = new_parent_id {
        if pid.0 == note_id {
            return Err(AppError::Other("A note cannot be its own parent".to_string()));
        }
        if is_descendant(conn, note_id, pid.0)? {
            return Err(AppError::Other("Circular parent-child relationship detected".to_string()));
        }
    }

    let title: String = conn.query_row(
        "SELECT title FROM notes WHERE note_id = ?",
        [note_id],
        |r| r.get(0)
    )?;

    let new_path = get_path_for_note(conn, new_parent_id, &title)?;

    conn.execute(
        "UPDATE notes SET parent_note_id = ?, path_cached = ?, updated_at = ? WHERE note_id = ?",
        rusqlite::params![new_parent_id.map(|id| id.0), new_path, updated_at, note_id],
    )?;

    recalculate_descendant_paths(conn, NoteId(note_id), &new_path)?;
    Ok(())
}

pub fn count_fragments(conn: &Connection, note_id: i64) -> Result<i64, AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM fragments WHERE note_id = ?",
        rusqlite::params![note_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

pub fn hard_delete(conn: &Connection, note_id: i64) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM notes WHERE note_id = ?",
        rusqlite::params![note_id],
    )?;
    Ok(())
}

pub fn get_all(conn: &Connection) -> Result<Vec<NoteListItem>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT note_id, title, is_deleted, embedding_model, indexing_version FROM notes",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(NoteListItem {
            note_id: NoteId(row.get(0)?),
            title: row.get(1)?,
            is_deleted: row.get::<_, i64>(2)? != 0,
            embedding_model: row.get(3)?,
            indexing_version: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn update_content_with_diff(
    conn: &mut Connection,
    note_id: i64,
    new_content: &str,
) -> Result<(), AppError> {
    let now = crate::db::time::now_seconds();
    let tx = conn.transaction()?;

    let old_content: String = tx.query_row(
        "SELECT content FROM notes WHERE note_id = ?",
        [note_id],
        |row| row.get(0)
    ).unwrap_or_default();

    if old_content != new_content {
        let patch = diffy::create_patch(&old_content, new_content);
        let patch_text = patch.to_string();

        if !patch_text.is_empty() {
            tx.execute(
                "INSERT INTO note_revisions (note_id, patch, created_at) VALUES (?, ?, ?)",
                rusqlite::params![note_id, patch_text, now],
            )?;
        }

        let content_hash = blake3::hash(new_content.as_bytes()).to_hex().to_string();

        tx.execute(
            "UPDATE notes SET content = ?, content_hash = ?, indexing_status = 'stale', updated_at = ? WHERE note_id = ?",
            rusqlite::params![new_content, content_hash, now, note_id],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn set_content_hash(conn: &Connection, note_id: i64, hash: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET content_hash = ? WHERE note_id = ?",
        rusqlite::params![hash, note_id],
    )?;
    Ok(())
}

pub fn set_status(conn: &Connection, note_id: i64, status: IndexingStatus) -> Result<(), AppError> {
    conn.execute(
        "UPDATE notes SET indexing_status = ? WHERE note_id = ?",
        rusqlite::params![status.as_str(), note_id],
    )?;
    Ok(())
}

pub fn set_content(conn: &Connection, note_id: i64, content: &str) -> Result<(), AppError> {
    let now = crate::db::time::now_seconds();
    let content_hash = blake3::hash(content.as_bytes()).to_hex().to_string();
    conn.execute(
        "UPDATE notes SET content = ?, content_hash = ?, indexing_status = 'stale', updated_at = ? WHERE note_id = ?",
        rusqlite::params![content, content_hash, now, note_id],
    )?;
    Ok(())
}

pub fn archive_note(conn: &Connection, note_id: i64) -> Result<(), AppError> {
    let now = crate::db::time::now_seconds();
    conn.execute(
        "UPDATE notes SET is_draft = 0, is_archived = 1, updated_at = ? WHERE note_id = ?",
        rusqlite::params![now, note_id],
    )?;
    Ok(())
}

