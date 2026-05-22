use regex::Regex;

pub fn remove_list_markers(text: &str) -> String {
    let re = Regex::new(r"(?m)^[\s]*[-+*]\s+").unwrap();
    re.replace_all(text, "").to_string()
}
