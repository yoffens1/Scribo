use regex::Regex;

pub fn remove_empty_lines(text: &str) -> String {
    let re = Regex::new(r"\n\s*\n").unwrap();
    re.replace_all(text, "\n").to_string()
}
