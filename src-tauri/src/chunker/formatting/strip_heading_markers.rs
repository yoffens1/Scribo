use regex::Regex;

pub fn strip_heading_markers(text: &str) -> String {
    let re = Regex::new(r"(?m)^#{1,6}\s+").unwrap();
    re.replace_all(text, "").to_string()
}
