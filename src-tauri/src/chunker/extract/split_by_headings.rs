use regex::Regex;

pub fn split_by_headings(text: &str, level: usize) -> Vec<String> {
    let pattern = if (1..=6).contains(&level) {
        format!(r"^#{{{}}}\s", level)
    } else {
        r"^#{1,6}\s".to_string()
    };
    let heading_regex = Regex::new(&pattern).unwrap();

    let lines: Vec<&str> = text.split('\n').collect();
    let mut sections = Vec::new();
    let mut current = Vec::new();

    for line in lines {
        if heading_regex.is_match(line) && !line.trim().starts_with('|') {
            if !current.is_empty() {
                sections.push(current.join("\n"));
            }
            current = vec![line];
        } else {
            current.push(line);
        }
    }

    if !current.is_empty() {
        sections.push(current.join("\n"));
    }

    sections
}
