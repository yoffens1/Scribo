use regex::Regex;
use std::sync::LazyLock;
use crate::fragmenter::token::count_tokens;

static RE_SEP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\|[-:\s]*---").unwrap());
static RE_CHARS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[|\s\-:]+$").unwrap());
static RE_HYPHENS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":?-{3,}:?").unwrap());
pub static RE_TABLE_PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{TABLE_\d+\}\}").unwrap());
pub static RE_HEADING_RESTORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#{1,6}\s").unwrap());

#[derive(Debug, Clone)]
pub struct TableInfo {
    pub placeholder: String,
    pub content: String,
    pub tokens: usize,
}

pub fn extract_tables(text: &str) -> (String, Vec<TableInfo>) {
    let mut tables = Vec::new();
    let lines: Vec<&str> = text.split('\n').collect();
    let mut new_lines = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if line.trim().starts_with('|') && line.contains('|') {
            let mut table_lines = vec![line];
            let mut j = i + 1;

            while j < lines.len() && lines[j].trim().starts_with('|') && lines[j].contains('|') {
                table_lines.push(lines[j]);
                j += 1;
            }

            let has_separator = table_lines.iter().any(|l| RE_SEP.is_match(l));
            if has_separator {
                let placeholder = format!("{{{{TABLE_{}}}}}", tables.len());
                let table_content = table_lines.join("\n");

                tables.push(TableInfo {
                    tokens: count_tokens(&table_content),
                    placeholder: placeholder.clone(),
                    content: table_content,
                });

                new_lines.push(placeholder);
                i = j;
                continue;
            }
        }

        new_lines.push(line.to_string());
        i += 1;
    }

    (new_lines.join("\n"), tables)
}

fn parse_table_row(row: &str) -> Vec<String> {
    let mut cleaned = row.trim();
    if cleaned.starts_with('|') {
        cleaned = &cleaned[1..];
    }
    if cleaned.ends_with('|') {
        cleaned = &cleaned[..cleaned.len() - 1];
    }
    cleaned.split('|').map(|cell| cell.trim().to_string()).collect()
}

fn is_separator_row(row: &str) -> bool {
    let row_trim = row.trim();
    if !row_trim.starts_with('|') || !row_trim.ends_with('|') {
        return false;
    }
    
    if !RE_CHARS.is_match(row_trim) {
        return false;
    }
    
    let parts: Vec<&str> = row_trim.split('|').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return false;
    }
    
    parts.iter().all(|p| RE_HYPHENS.is_match(p.trim()))
}

pub fn linearize_table(table_text: &str) -> Vec<String> {
    let lines: Vec<&str> = table_text.split('\n').filter(|l| !l.trim().is_empty()).collect();
    if lines.len() < 2 {
        return vec![table_text.to_string()];
    }

    let mut separator_index = None;
    for (idx, line) in lines.iter().enumerate() {
        if is_separator_row(line) {
            separator_index = Some(idx);
            break;
        }
    }

    let sep_idx = match separator_index {
        Some(idx) => idx,
        None => return vec![table_text.to_string()],
    };

    if sep_idx == 0 {
        return vec![table_text.to_string()];
    }

    let header_line = lines[sep_idx - 1];
    let headers = parse_table_row(header_line);

    let data_rows = &lines[sep_idx + 1..];
    let mut result = Vec::new();

    for row in data_rows {
        let cells = parse_table_row(row);
        if cells.is_empty() {
            continue;
        }

        let mut parts = Vec::new();
        for i in 0..headers.len() {
            if i < cells.len() {
                let header = &headers[i];
                let value = &cells[i];
                if !value.is_empty() {
                    parts.push(format!("{}: {}", header, value));
                }
            }
        }

        if !parts.is_empty() {
            result.push(parts.join(". "));
        }
    }

    let numbered: Vec<String> = result
        .into_iter()
        .enumerate()
        .map(|(idx, desc)| format!("{}. {}", idx + 1, desc))
        .collect();

    if numbered.is_empty() {
        vec![table_text.to_string()]
    } else {
        numbered
    }
}


pub fn split_fragment_around_tables(fragment: &str, fragment_tables: &[&TableInfo]) -> Vec<String> {
    let mut parts = Vec::new();
    let mut remaining = fragment;

    for t in fragment_tables {
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

pub fn partition_table_lines(fragment: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    if !fragment.contains('|') {
        return (fragment.lines().map(|s| s.to_string()).collect(), Vec::new(), Vec::new());
    }

    let lines = fragment.lines();
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

