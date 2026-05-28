use regex::Regex;
use std::sync::LazyLock;
use crate::fragmenter::config::CleanFlags;
use crate::fragmenter::token::count_tokens;
use crate::fragmenter::pack::RawFragment;

static RE_SEP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\|[-:\s]*---").unwrap());
static RE_CHARS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[|\s\-:]+$").unwrap());
static RE_HYPHENS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":?-{3,}:?").unwrap());
static RE_TABLE_PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{TABLE_\d+\}\}").unwrap());
static RE_HEADING_RESTORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*#{1,6}\s").unwrap());

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

pub fn restore_tables(
    raw_fragments: Vec<RawFragment>,
    tables: &[TableInfo],
    flags: &CleanFlags,
) -> Vec<RawFragment> {
    let mut used = std::collections::HashSet::new();
    let mut result = Vec::new();

    for fragment in raw_fragments {
        let mut fragment_tables = Vec::new();
        for cap in RE_TABLE_PLACEHOLDER.captures_iter(&fragment.text) {
            let placeholder = cap[0].to_string();
            if let Some(t) = tables.iter().find(|t| t.placeholder == placeholder) {
                if !fragment_tables.iter().any(|existing: &&TableInfo| existing.placeholder == placeholder) {
                    fragment_tables.push(t);
                }
            }
        }

        for t in &fragment_tables {
            used.insert(t.placeholder.clone());
        }

        if flags.separate_tables_as_fragments && !fragment_tables.is_empty() {
            let split_texts = split_fragment_around_tables(&fragment.text, &fragment_tables);
            for t in split_texts {
                result.push(RawFragment {
                    text: t,
                    meta: fragment.meta.clone(),
                });
            }
        } else {
            let mut restored = fragment.text.clone();
            for t in fragment_tables {
                restored = restored.replace(&t.placeholder, &t.content);
            }
            result.push(RawFragment {
                text: restored,
                meta: fragment.meta.clone(),
            });
        }
    }

    for t in tables {
        if !used.contains(&t.placeholder) {
            result.push(RawFragment {
                text: t.content.clone(),
                meta: Default::default(),
            });
        }
    }

    result
        .into_iter()
        .filter(|fragment| {
            let lines: Vec<&str> = fragment.text.lines().filter(|l| !l.trim().is_empty()).collect();
            !lines.iter().all(|line| RE_HEADING_RESTORE.is_match(line))
        })
        .collect()
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

pub fn linearize_table_fragments(
    fragments: Vec<RawFragment>,
    flags: &CleanFlags,
    max_tokens: usize,
) -> Vec<RawFragment> {
    if !flags.linearize_tables {
        return fragments;
    }

    let mut result = Vec::new();
    for fragment in fragments {
        let (before, table_block, after) = partition_table_lines(&fragment.text);
        if table_block.is_empty() {
            result.push(fragment);
            continue;
        }

        let table_text = table_block.join("\n");
        let mut rows = linearize_table(&table_text);

        let mut row_clean_flags = flags.clone();
        row_clean_flags.lower_case = false;
        row_clean_flags.compact_lines = false;
        row_clean_flags.strip_heading_markers = false;
        rows = rows.iter().map(|row| super::transforms::apply(row, &row_clean_flags)).collect();

        let sub_fragments_text = if flags.each_table_row_as_separate_fragment {
            rows
        } else {
            // We can pack them back using a token batch aggregator
            let sub_atoms = rows.iter().map(|r| crate::fragmenter::segment::Atom::whole(r)).collect();
            let packed = crate::fragmenter::pack::token_budget::pack(sub_atoms, max_tokens, 0);
            packed.into_iter().map(|rf| rf.text).collect()
        };

        let mut sub_fragments: Vec<RawFragment> = sub_fragments_text
            .into_iter()
            .map(|t| RawFragment {
                text: t,
                meta: fragment.meta.clone(),
            })
            .collect();

        if !before.is_empty() && !sub_fragments.is_empty() {
            sub_fragments[0].text = format!("{}\n{}", before.join("\n"), sub_fragments[0].text);
        } else if !before.is_empty() {
            sub_fragments.push(RawFragment {
                text: before.join("\n"),
                meta: fragment.meta.clone(),
            });
        }

        if !after.is_empty() && !sub_fragments.is_empty() {
            let last_idx = sub_fragments.len() - 1;
            sub_fragments[last_idx].text = format!("{}\n{}", sub_fragments[last_idx].text, after.join("\n"));
        } else if !after.is_empty() {
            sub_fragments.push(RawFragment {
                text: after.join("\n"),
                meta: fragment.meta.clone(),
            });
        }

        if sub_fragments.is_empty() {
            result.push(fragment);
        } else {
            result.extend(sub_fragments);
        }
    }
    result
}
