use regex::Regex;
use crate::chunker::types::TableInfo;
use crate::chunker::token::count_tokens;
use std::sync::LazyLock;

static RE_SEP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\|[-:\s]*---").unwrap());

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
