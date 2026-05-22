use regex::Regex;

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
    let trimmed = row.trim();
    let re_chars = Regex::new(r"^[|\s\-:]+$").unwrap();
    let re_hyphens = Regex::new(r":?-{3,}:?").unwrap();
    re_chars.is_match(trimmed) && re_hyphens.is_match(trimmed)
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
