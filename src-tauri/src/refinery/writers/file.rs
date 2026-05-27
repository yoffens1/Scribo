use std::path::Path;
use tokio::fs;
use crate::DbState;
use crate::ai::LlmService;
use crate::ai::types::Message;
use crate::refinery::types::WriteOperation;
use crate::db::hash::content_hash;
use std::sync::Arc;

pub struct FileWriterContext {
    pub llm: Option<Arc<LlmService>>,
    pub output_root: String,
    pub overwrite_on_merge: bool,
    pub merge_tags: bool,
    pub delete_from_db_on_gc: bool,
}

pub struct FileWriter {
    ctx: FileWriterContext,
}

impl FileWriter {
    pub fn new(ctx: FileWriterContext) -> Self {
        Self { ctx }
    }

    pub async fn execute(&self, op: &WriteOperation, _source_note_id: Option<i64>, db_state: Option<&DbState>) -> Result<(), String> {
        match op {
            WriteOperation::CreateFile { path, content } => {
                if let Some(parent) = Path::new(path).parent() {
                    fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
                }
                fs::write(path, content).await.map_err(|e| e.to_string())?;
                self.sync_database(path, content, db_state).await?;
            }
            WriteOperation::MergeFragment { source_file: _, target_file, fragment_text } => {
                let merged_content = self.merge_card_content(target_file, fragment_text).await?;
                if let Some(parent) = Path::new(target_file).parent() {
                    fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
                }
                fs::write(target_file, &merged_content).await.map_err(|e| e.to_string())?;
                self.sync_database(target_file, &merged_content, db_state).await?;
            }
            WriteOperation::CreateFolder { path } => {
                fs::create_dir_all(path).await.map_err(|e| e.to_string())?;
            }
            WriteOperation::MoveFile { from, to } => {
                if let Some(parent) = Path::new(to).parent() {
                    fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
                }
                fs::rename(from, to).await.map_err(|e| e.to_string())?;
                if let Some(state) = db_state {
                    let old_title = Path::new(from).file_stem().unwrap_or_default().to_string_lossy().into_owned();
                    let new_title = Path::new(to).file_stem().unwrap_or_default().to_string_lossy().into_owned();
                    let _ = state.with_conn(|conn| {
                        conn.execute("UPDATE notes SET title = ? WHERE title = ?", [new_title, old_title])?;
                        Ok(())
                    });
                }
            }
            WriteOperation::DeleteFile { path } => {
                let _ = fs::remove_file(path).await;
                if let Some(state) = db_state {
                    let title = Path::new(path).file_stem().unwrap_or_default().to_string_lossy().into_owned();
                    let _ = state.with_conn(|conn| {
                        if self.ctx.delete_from_db_on_gc {
                            conn.execute("DELETE FROM notes WHERE title = ?", [&title])?;
                        } else {
                            conn.execute("UPDATE notes SET is_deleted = 1 WHERE title = ?", [&title])?;
                        }
                        Ok(())
                    });
                }
            }
        }
        Ok(())
    }

    async fn merge_card_content(&self, target_path: &str, new_content: &str) -> Result<String, String> {
        let existing = fs::read_to_string(target_path).await.unwrap_or_default();
        if existing.trim().is_empty() {
            return Ok(new_content.to_string());
        }

        let (ex_fm, ex_body) = Self::split_frontmatter(&existing);
        let (new_fm, new_body) = Self::split_frontmatter(new_content);

        let merged_body = if self.ctx.overwrite_on_merge {
            if let Some(ref llm) = self.ctx.llm {
                let messages = vec![
                    Message {
                        role: "system".to_string(),
                        content: "Merge the two notes into a single cohesive markdown note. Keep all facts and details. Avoid repetition.".to_string(),
                    },
                    Message {
                        role: "user".to_string(),
                        content: format!("Existing Note:\n{}\n\nNew Note to merge:\n{}", ex_body, new_body),
                    }
                ];
                if let Ok(resp) = llm.generate_messages(messages).await {
                    resp.text.trim().to_string()
                } else {
                    format!("{}\n\n{}", ex_body.trim(), new_body.trim())
                }
            } else {
                format!("{}\n\n{}", ex_body.trim(), new_body.trim())
            }
        } else {
            format!("{}\n\n{}", ex_body.trim(), new_body.trim())
        };

        let merged_fm = if self.ctx.merge_tags {
            let mut ex_aliases = Self::parse_list(&ex_fm, "aliases");
            let mut new_aliases = Self::parse_list(&new_fm, "aliases");
            ex_aliases.append(&mut new_aliases);
            ex_aliases.sort(); ex_aliases.dedup();

            let mut ex_tags = Self::parse_list(&ex_fm, "tags");
            let mut new_tags = Self::parse_list(&new_fm, "tags");
            ex_tags.append(&mut new_tags);
            ex_tags.sort(); ex_tags.dedup();

            let mut ex_sources = Self::parse_list(&ex_fm, "sources");
            let mut new_sources = Self::parse_list(&new_fm, "sources");
            ex_sources.append(&mut new_sources);
            ex_sources.sort(); ex_sources.dedup();

            let mut lines = vec!["---".to_string()];
            if !ex_aliases.is_empty() {
                lines.push(format!("aliases: [{}]", ex_aliases.into_iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ")));
            }
            if !ex_tags.is_empty() {
                lines.push(format!("tags: [{}]", ex_tags.into_iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ")));
            }
            if !ex_sources.is_empty() {
                lines.push(format!("sources: [{}]", ex_sources.into_iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ")));
            }
            lines.push("---".to_string());
            if lines.len() > 2 {
                lines.join("\n") + "\n"
            } else {
                String::new()
            }
        } else {
            if !new_fm.is_empty() {
                format!("---\n{}\n---\n", new_fm)
            } else {
                String::new()
            }
        };

        if merged_fm.is_empty() {
            Ok(merged_body)
        } else {
            Ok(format!("{}\n{}", merged_fm, merged_body))
        }
    }

    fn split_frontmatter(text: &str) -> (String, String) {
        if text.starts_with("---\n") {
            if let Some(end_idx) = text[4..].find("\n---\n") {
                let fm = &text[4..4+end_idx];
                let body = &text[4+end_idx+5..];
                return (fm.to_string(), body.to_string());
            }
        }
        (String::new(), text.to_string())
    }

    fn parse_list(fm: &str, key: &str) -> Vec<String> {
        for line in fm.lines() {
            if line.starts_with(key) && line[key.len()..].starts_with(':') {
                let val = line[key.len()+1..].trim();
                let val = val.trim_start_matches('[').trim_end_matches(']');
                if val.is_empty() {
                    continue;
                }
                return val.split(',')
                    .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
        Vec::new()
    }

    async fn sync_database(&self, file_path: &str, content: &str, db_state: Option<&DbState>) -> Result<(), String> {
        if let Some(state) = db_state {
            let file_hash = content_hash(content);
            let file_name = Path::new(file_path).file_name().unwrap_or_default().to_string_lossy().into_owned();
            let title = Path::new(&file_name).file_stem().unwrap_or_default().to_string_lossy().into_owned();
            
            let mtime = crate::db::time::now_seconds();

            let _ = state.with_conn(move |conn| {
                let mut stmt = conn.prepare("SELECT note_id FROM notes WHERE title = ?")?;
                let mut rows = stmt.query([&title])?;
                let _note_id = if let Some(row) = rows.next()? {
                    let id: i64 = row.get(0)?;
                    conn.execute(
                        "UPDATE notes SET content = ?, content_hash = ?, is_deleted = 0, indexing_status = 'indexed', updated_at = ? WHERE note_id = ?",
                        (content, &file_hash, mtime, id)
                    )?;
                    id
                } else {
                    conn.execute(
                        "INSERT INTO notes (title, content, content_hash, is_deleted, indexing_status, updated_at) VALUES (?, ?, ?, 0, 'indexed', ?)",
                        (&title, content, &file_hash, mtime)
                    )?;
                    conn.last_insert_rowid()
                };

                Ok(())
            });
        }
        Ok(())
    }
}
