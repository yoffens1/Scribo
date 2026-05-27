use rusqlite::{Connection, OptionalExtension};
use crate::error::AppError;
use crate::domain::tag::{Tag, TagId, TagSource};
use crate::domain::note::NoteId;
use regex::Regex;

pub fn slugify(name: &str) -> String {
    name.trim().to_lowercase()
}

pub fn extract_raw_tags(input: &str) -> Vec<String> {
    thread_local! {
        static TAG_RE: Regex = Regex::new(r"#([a-zA-Z0-9_\-/]+)").unwrap();
    }
    TAG_RE.with(|re| {
        re.captures_iter(input)
            .map(|cap| cap[1].to_string())
            .collect()
    })
}

fn row_to_tag(row: &rusqlite::Row) -> rusqlite::Result<Tag> {
    let parent_tag_id: Option<i64> = row.get(1)?;
    Ok(Tag {
        tag_id: TagId(row.get(0)?),
        parent_tag_id: parent_tag_id.map(TagId),
        name: row.get(2)?,
        slug: row.get(3)?,
        color: row.get(4)?,
        icon: row.get(5)?,
        depth: row.get(6)?,
        path_cached: row.get(7)?,
        description: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

pub fn get_by_id(conn: &Connection, tag_id: TagId) -> Result<Option<Tag>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT tag_id, parent_tag_id, name, slug, color, icon, depth, path_cached, description, created_at, updated_at
         FROM tags
         WHERE tag_id = ?"
    )?;
    let tag = stmt.query_row([tag_id.0], row_to_tag).optional()?;
    Ok(tag)
}

pub fn get_by_path(conn: &Connection, path_cached: &str) -> Result<Option<Tag>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT tag_id, parent_tag_id, name, slug, color, icon, depth, path_cached, description, created_at, updated_at
         FROM tags
         WHERE path_cached = ?"
    )?;
    let tag = stmt.query_row([path_cached], row_to_tag).optional()?;
    Ok(tag)
}

pub fn create_tag(
    conn: &Connection,
    name: &str,
    parent_id: Option<TagId>,
) -> Result<TagId, AppError> {
    let slug = slugify(name);
    let now = crate::db::time::now_seconds();
    
    let (depth, path_cached) = match parent_id {
        None => (0, slug.clone()),
        Some(pid) => {
            let (parent_depth, parent_path): (i64, String) = conn.query_row(
                "SELECT depth, path_cached FROM tags WHERE tag_id = ?",
                [pid.0],
                |row| Ok((row.get(0)?, row.get(1)?))
            )?;
            (parent_depth + 1, format!("{}/{}", parent_path, slug))
        }
    };
    
    let tag_id: i64 = conn.query_row(
        "INSERT INTO tags (
            parent_tag_id, name, slug, depth, path_cached, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?)
         RETURNING tag_id",
        rusqlite::params![
            parent_id.map(|id| id.0),
            name.trim(),
            slug,
            depth,
            path_cached,
            now,
            now
        ],
        |row| row.get(0)
    )?;
    
    // Insert into tag_closure self
    conn.execute(
        "INSERT INTO tag_closure (ancestor_id, descendant_id, depth) VALUES (?, ?, 0)",
        [tag_id, tag_id]
    )?;
    
    // Insert into tag_closure ancestor paths
    if let Some(pid) = parent_id {
        conn.execute(
            "INSERT INTO tag_closure (ancestor_id, descendant_id, depth)
             SELECT ancestor_id, ?, depth + 1
             FROM tag_closure
             WHERE descendant_id = ?",
            [tag_id, pid.0]
        )?;
    }
    
    Ok(TagId(tag_id))
}

pub fn delete_tag(conn: &Connection, tag_id: TagId) -> Result<(), AppError> {
    // ON DELETE CASCADE automatically deletes children tags, note_tags, chunk_tags and tag_closure entries.
    conn.execute("DELETE FROM tags WHERE tag_id = ?", [tag_id.0])?;
    Ok(())
}

fn recalculate_descendant_tag_paths(
    conn: &Connection,
    parent_id: TagId,
    parent_path: &str,
    parent_depth: i64,
) -> Result<(), AppError> {
    let mut stmt = conn.prepare("SELECT tag_id, slug FROM tags WHERE parent_tag_id = ?")?;
    let mut rows = stmt.query([parent_id.0])?;
    let mut children = Vec::new();
    while let Some(row) = rows.next()? {
        let child_id: i64 = row.get(0)?;
        let child_slug: String = row.get(1)?;
        children.push((child_id, child_slug));
    }
    for (child_id, child_slug) in children {
        let child_path = format!("{}/{}", parent_path, child_slug);
        let child_depth = parent_depth + 1;
        conn.execute(
            "UPDATE tags SET path_cached = ?, depth = ? WHERE tag_id = ?",
            rusqlite::params![child_path, child_depth, child_id],
        )?;
        recalculate_descendant_tag_paths(conn, TagId(child_id), &child_path, child_depth)?;
    }
    Ok(())
}

pub fn rename_tag(conn: &Connection, id: TagId, new_name: &str) -> Result<(), AppError> {
    let slug = slugify(new_name);
    let now = crate::db::time::now_seconds();
    
    let (parent_id, depth): (Option<i64>, i64) = conn.query_row(
        "SELECT parent_tag_id, depth FROM tags WHERE tag_id = ?",
        [id.0],
        |row| Ok((row.get(0)?, row.get(1)?))
    )?;
    
    let path_cached = match parent_id {
        None => slug.clone(),
        Some(pid) => {
            let parent_path: String = conn.query_row(
                "SELECT path_cached FROM tags WHERE tag_id = ?",
                [pid],
                |row| row.get(0)
            )?;
            format!("{}/{}", parent_path, slug)
        }
    };
    
    conn.execute(
        "UPDATE tags SET name = ?, slug = ?, path_cached = ?, updated_at = ? WHERE tag_id = ?",
        rusqlite::params![new_name.trim(), slug, path_cached, now, id.0]
    )?;
    
    recalculate_descendant_tag_paths(conn, id, &path_cached, depth)?;
    Ok(())
}

pub fn move_tag(conn: &Connection, tag_id: TagId, new_parent_id: Option<TagId>) -> Result<(), AppError> {
    let now = crate::db::time::now_seconds();
    
    // Get slug and depth
    let (slug, _depth): (String, i64) = conn.query_row(
        "SELECT slug, depth FROM tags WHERE tag_id = ?",
        [tag_id.0],
        |row| Ok((row.get(0)?, row.get(1)?))
    )?;
    
    // Check for cycles
    if let Some(np_id) = new_parent_id {
        if tag_id.0 == np_id.0 {
            return Err(AppError::Other("Cannot move tag to itself".to_string()));
        }
        let is_descendant: bool = conn.query_row(
            "SELECT 1 FROM tag_closure WHERE ancestor_id = ? AND descendant_id = ?",
            [tag_id.0, np_id.0],
            |_| Ok(true)
        ).optional()?.unwrap_or(false);
        if is_descendant {
            return Err(AppError::Other("Cycle detected: cannot move tag to its own descendant".to_string()));
        }
    }
    
    // Remove old ancestor relations from the closure table
    conn.execute(
        "DELETE FROM tag_closure
         WHERE descendant_id IN (SELECT descendant_id FROM tag_closure WHERE ancestor_id = ?)
           AND ancestor_id NOT IN (SELECT descendant_id FROM tag_closure WHERE ancestor_id = ?)",
        [tag_id.0, tag_id.0]
    )?;
    
    // Insert new ancestor relations into the closure table
    if let Some(np_id) = new_parent_id {
        conn.execute(
            "INSERT INTO tag_closure (ancestor_id, descendant_id, depth)
             SELECT a.ancestor_id, d.descendant_id, a.depth + d.depth + 1
             FROM tag_closure a
             CROSS JOIN tag_closure d
             WHERE a.descendant_id = ?
               AND d.ancestor_id = ?",
            [np_id.0, tag_id.0]
        )?;
    }
    
    // Update tag's parent_tag_id
    conn.execute(
        "UPDATE tags SET parent_tag_id = ?, updated_at = ? WHERE tag_id = ?",
        rusqlite::params![new_parent_id.map(|id| id.0), now, tag_id.0]
    )?;
    
    // Recalculate depth and path_cached
    let (new_depth, new_path) = match new_parent_id {
        None => (0, slug.clone()),
        Some(pid) => {
            let (parent_depth, parent_path): (i64, String) = conn.query_row(
                "SELECT depth, path_cached FROM tags WHERE tag_id = ?",
                [pid.0],
                |row| Ok((row.get(0)?, row.get(1)?))
            )?;
            (parent_depth + 1, format!("{}/{}", parent_path, slug))
        }
    };
    
    conn.execute(
        "UPDATE tags SET depth = ?, path_cached = ? WHERE tag_id = ?",
        rusqlite::params![new_depth, new_path, tag_id.0]
    )?;
    
    recalculate_descendant_tag_paths(conn, tag_id, &new_path, new_depth)?;
    Ok(())
}

pub fn parse_and_resolve_tags(conn: &Connection, input: &str) -> Result<Vec<TagId>, AppError> {
    let raw_tags = extract_raw_tags(input);
    let mut result = Vec::new();
    for raw in raw_tags {
        let parts: Vec<&str> = raw.split('/').collect();
        let mut parent_id: Option<TagId> = None;
        for part in parts {
            let part_trimmed = part.trim();
            if part_trimmed.is_empty() {
                continue;
            }
            let slug = slugify(part_trimmed);
            
            let query = "SELECT tag_id FROM tags WHERE parent_tag_id IS ? AND slug = ?";
            let existing_id: Option<i64> = conn.query_row(
                query,
                rusqlite::params![parent_id.map(|id| id.0), slug],
                |row| row.get(0)
            ).optional()?;
            
            let tag_id = match existing_id {
                Some(id) => TagId(id),
                None => {
                    create_tag(conn, part_trimmed, parent_id)?
                }
            };
            parent_id = Some(tag_id);
        }
        if let Some(leaf_id) = parent_id {
            result.push(leaf_id);
        }
    }
    Ok(result)
}

pub fn associate_note_tag(
    conn: &Connection,
    note_id: NoteId,
    tag_id: TagId,
    source: TagSource,
    confidence: Option<f64>,
) -> Result<(), AppError> {
    let now = crate::db::time::now_seconds();
    conn.execute(
        "INSERT OR REPLACE INTO note_tags (note_id, tag_id, source, confidence, created_at)
         VALUES (?, ?, ?, ?, ?)",
        rusqlite::params![note_id.0, tag_id.0, source.to_string(), confidence, now],
    )?;
    Ok(())
}

pub fn dissociate_note_tag(
    conn: &Connection,
    note_id: NoteId,
    tag_id: TagId,
) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM note_tags WHERE note_id = ? AND tag_id = ?",
        [note_id.0, tag_id.0],
    )?;
    Ok(())
}

pub fn get_note_tags(conn: &Connection, note_id: NoteId) -> Result<Vec<Tag>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT t.tag_id, t.parent_tag_id, t.name, t.slug, t.color, t.icon, t.depth, t.path_cached, t.description, t.created_at, t.updated_at
         FROM tags t
         JOIN note_tags nt ON nt.tag_id = t.tag_id
         WHERE nt.note_id = ?"
    )?;
    let rows = stmt.query_map([note_id.0], row_to_tag)?;
    let mut res = Vec::new();
    for r in rows {
        res.push(r?);
    }
    Ok(res)
}

pub fn get_note_ids_by_tag(
    conn: &Connection,
    tag_path: &str,
    include_subtree: bool,
) -> Result<Vec<i64>, AppError> {
    let sql = if include_subtree {
        "SELECT DISTINCT nt.note_id
         FROM note_tags nt
         JOIN tag_closure tc ON tc.descendant_id = nt.tag_id
         WHERE tc.ancestor_id = (SELECT tag_id FROM tags WHERE path_cached = ?)"
    } else {
        "SELECT nt.note_id
         FROM note_tags nt
         WHERE nt.tag_id = (SELECT tag_id FROM tags WHERE path_cached = ?)"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([tag_path], |row| row.get(0))?;
    let mut ids = Vec::new();
    for r in rows {
        ids.push(r?);
    }
    Ok(ids)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutocompleteTagResult {
    pub tag_id: TagId,
    pub name: String,
    pub path_cached: String,
    pub depth: i64,
}

pub fn autocomplete_tags(
    conn: &Connection,
    prefix: &str,
    limit: i64,
) -> Result<Vec<AutocompleteTagResult>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT tag_id, name, path_cached, depth
         FROM tags
         WHERE path_cached LIKE ? OR slug LIKE ?
         ORDER BY 
           (SELECT COUNT(*) FROM note_tags WHERE tag_id = tags.tag_id) DESC,
           depth ASC
         LIMIT ?"
    )?;
    let pattern = format!("{}%", prefix.to_lowercase());
    let rows = stmt.query_map(rusqlite::params![pattern, pattern, limit], |row| {
        Ok(AutocompleteTagResult {
            tag_id: TagId(row.get(0)?),
            name: row.get(1)?,
            path_cached: row.get(2)?,
            depth: row.get(3)?,
        })
    })?;
    let mut res = Vec::new();
    for r in rows {
        res.push(r?);
    }
    Ok(res)
}

pub fn inherit_note_tags_to_chunks(
    conn: &Connection,
    note_id: NoteId,
    tag_id: TagId,
) -> Result<(), AppError> {
    let now = crate::db::time::now_seconds();
    conn.execute(
        "INSERT INTO chunk_tags (chunk_id, tag_id, source, created_at)
         SELECT c.chunk_id, ?, 'inherited', ?
         FROM chunks c
         WHERE c.note_id = ?
           AND NOT EXISTS (
             SELECT 1 FROM chunk_tags 
             WHERE chunk_id = c.chunk_id AND tag_id = ?
           )",
        rusqlite::params![tag_id.0, now, note_id.0, tag_id.0]
    )?;
    Ok(())
}

pub fn remove_inherited_note_tags_from_chunks(
    conn: &Connection,
    note_id: NoteId,
    tag_id: TagId,
) -> Result<(), AppError> {
    conn.execute(
        "DELETE FROM chunk_tags
         WHERE tag_id = ?
           AND source = 'inherited'
           AND chunk_id IN (SELECT chunk_id FROM chunks WHERE note_id = ?)",
        rusqlite::params![tag_id.0, note_id.0]
    )?;
    Ok(())
}
