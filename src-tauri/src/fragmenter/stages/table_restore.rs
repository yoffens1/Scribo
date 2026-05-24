use std::sync::LazyLock;
use crate::fragmenter::types;
use crate::fragmenter::types::FragmentOptions;
use crate::fragmenter::markdown::table;
use super::clean::clean_fragment;
use super::assemble::assemble_raw_fragments;

static RE_TABLE_PLACEHOLDER: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\{\{TABLE_\d+\}\}").unwrap());
static RE_HEADING_RESTORE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"^\s*#{1,6}\s").unwrap());

pub fn restore_tables(raw_fragments: Vec<String>, tables: &[types::TableInfo], options: &FragmentOptions) -> Vec<String> {
    let mut used = std::collections::HashSet::new();
    let mut result = Vec::new();

    for fragment in raw_fragments {
        let mut fragment_tables = Vec::new();
        for cap in RE_TABLE_PLACEHOLDER.captures_iter(&fragment) {
            let placeholder = cap[0].to_string();
            if let Some(t) = tables.iter().find(|t| t.placeholder == placeholder) {
                if !fragment_tables.iter().any(|existing: &&types::TableInfo| existing.placeholder == placeholder) {
                    fragment_tables.push(t);
                }
            }
        }

        for t in &fragment_tables {
            used.insert(t.placeholder.clone());
        }

        if options.separate_tables_as_fragments && !fragment_tables.is_empty() {
            result.extend(split_fragment_around_tables(&fragment, &fragment_tables));
        } else {
            let mut restored = fragment.clone();
            for t in fragment_tables {
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
        .filter(|fragment| {
            let lines: Vec<&str> = fragment.lines().filter(|l| !l.trim().is_empty()).collect();
            !lines.iter().all(|line| RE_HEADING_RESTORE.is_match(line))
        })
        .collect()
}

pub fn split_fragment_around_tables(fragment: &str, fragment_tables: &[&types::TableInfo]) -> Vec<String> {
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

pub fn linearize_table_fragments(fragments: Vec<String>, options: &FragmentOptions) -> Vec<String> {
    if !options.linearize_tables {
        return fragments;
    }

    let mut result = Vec::new();
    for fragment in fragments {
        let (before, table_block, after) = partition_table_lines(&fragment);
        if table_block.is_empty() {
            result.push(fragment);
            continue;
        }

        let table_text = table_block.join("\n");
        let mut rows = table::linearize_table(&table_text);

        let mut clean_opts = options.clone();
        clean_opts.lower_case = false;
        clean_opts.compact_lines = false;
        clean_opts.strip_heading_markers = false;
        rows = rows.iter().map(|row| clean_fragment(row, &clean_opts)).collect();

        let mut sub_fragments = if options.each_table_row_as_separate_fragment {
            rows
        } else {
            let rows_cow: Vec<std::borrow::Cow<'_, str>> = rows.into_iter().map(std::borrow::Cow::Owned).collect();
            assemble_raw_fragments(rows_cow, options)
        };

        if !before.is_empty() && !sub_fragments.is_empty() {
            sub_fragments[0] = format!("{}\n{}", before.join("\n"), sub_fragments[0]);
        } else if !before.is_empty() {
            sub_fragments.push(before.join("\n"));
        }

        if !after.is_empty() && !sub_fragments.is_empty() {
            let last_idx = sub_fragments.len() - 1;
            sub_fragments[last_idx] = format!("{}\n{}", sub_fragments[last_idx], after.join("\n"));
        } else if !after.is_empty() {
            sub_fragments.push(after.join("\n"));
        }

        if sub_fragments.is_empty() {
            result.push(fragment);
        } else {
            result.extend(sub_fragments);
        }
    }
    result
}
