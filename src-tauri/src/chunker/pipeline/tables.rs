use std::sync::LazyLock;
use crate::chunker::types;
use crate::chunker::types::ChunkOptions;
use crate::chunker::table;
use super::clean::clean_chunk;
use super::assemble::assemble_raw_chunks;

static RE_TABLE_PLACEHOLDER: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\{\{TABLE_\d+\}\}").unwrap());
static RE_HEADING_RESTORE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"^\s*#{1,6}\s").unwrap());

pub fn restore_tables(raw_chunks: Vec<String>, tables: &[types::TableInfo], options: &ChunkOptions) -> Vec<String> {
    let mut used = std::collections::HashSet::new();
    let mut result = Vec::new();

    for chunk in raw_chunks {
        let mut chunk_tables = Vec::new();
        for cap in RE_TABLE_PLACEHOLDER.captures_iter(&chunk) {
            let placeholder = cap[0].to_string();
            if let Some(t) = tables.iter().find(|t| t.placeholder == placeholder) {
                if !chunk_tables.iter().any(|existing: &&types::TableInfo| existing.placeholder == placeholder) {
                    chunk_tables.push(t);
                }
            }
        }

        for t in &chunk_tables {
            used.insert(t.placeholder.clone());
        }

        if options.separate_tables_as_chunks && !chunk_tables.is_empty() {
            result.extend(split_chunk_around_tables(&chunk, &chunk_tables));
        } else {
            let mut restored = chunk.clone();
            for t in chunk_tables {
                restored = restored.replace(&t.placeholder, &t.content);
            }
            result.push(restored);
        }
    }

    for t in tables {
        if !used.contains(&t.placeholder) {
            result.push(t.content.clone());
        }
    }

    result
        .into_iter()
        .filter(|chunk| {
            let lines: Vec<&str> = chunk.lines().filter(|l| !l.trim().is_empty()).collect();
            !lines.iter().all(|line| RE_HEADING_RESTORE.is_match(line))
        })
        .collect()
}

pub fn split_chunk_around_tables(chunk: &str, chunk_tables: &[&types::TableInfo]) -> Vec<String> {
    let mut parts = Vec::new();
    let mut remaining = chunk;

    for t in chunk_tables {
        if let Some(idx) = remaining.find(&t.placeholder) {
            let before = remaining[..idx].trim();
            remaining = &remaining[idx + t.placeholder.len()..];

            if !before.is_empty() {
                parts.push(before.to_string());
            }
            parts.push(t.content.clone());
        }
    }

    let after = remaining.trim();
    if !after.is_empty() {
        parts.push(after.to_string());
    }

    parts
}

pub fn partition_table_lines(chunk: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    if !chunk.contains('|') {
        return (chunk.lines().map(|s| s.to_string()).collect(), Vec::new(), Vec::new());
    }

    let lines = chunk.lines();
    let mut before = Vec::new();
    let mut table_block = Vec::new();
    let mut after = Vec::new();
    let mut inside_table = false;
    let mut past_table = false;

    for line in lines {
        if past_table {
            after.push(line.to_string());
        } else if line.trim().starts_with('|') {
            table_block.push(line.to_string());
            inside_table = true;
        } else if inside_table {
            inside_table = false;
            past_table = true;
            after.push(line.to_string());
        } else {
            before.push(line.to_string());
        }
    }
    (before, table_block, after)
}

pub fn linearize_table_chunks(chunks: Vec<String>, options: &ChunkOptions) -> Vec<String> {
    if !options.linearize_tables {
        return chunks;
    }

    let mut result = Vec::new();
    for chunk in chunks {
        let (before, table_block, after) = partition_table_lines(&chunk);
        if table_block.is_empty() {
            result.push(chunk);
            continue;
        }

        let table_text = table_block.join("\n");
        let mut rows = table::linearize_table(&table_text);

        let mut clean_opts = options.clone();
        clean_opts.lower_case = false;
        clean_opts.compact_lines = false;
        clean_opts.strip_heading_markers = false;
        rows = rows.iter().map(|row| clean_chunk(row, &clean_opts)).collect();

        let mut sub_chunks = if options.each_table_row_as_separate_chunk {
            rows
        } else {
            let rows_cow: Vec<std::borrow::Cow<'_, str>> = rows.into_iter().map(std::borrow::Cow::Owned).collect();
            assemble_raw_chunks(rows_cow, options)
        };

        if !before.is_empty() && !sub_chunks.is_empty() {
            sub_chunks[0] = format!("{}\n{}", before.join("\n"), sub_chunks[0]);
        } else if !before.is_empty() {
            sub_chunks.push(before.join("\n"));
        }

        if !after.is_empty() && !sub_chunks.is_empty() {
            let last_idx = sub_chunks.len() - 1;
            sub_chunks[last_idx] = format!("{}\n{}", sub_chunks[last_idx], after.join("\n"));
        } else if !after.is_empty() {
            sub_chunks.push(after.join("\n"));
        }

        if sub_chunks.is_empty() {
            result.push(chunk);
        } else {
            result.extend(sub_chunks);
        }
    }
    result
}
