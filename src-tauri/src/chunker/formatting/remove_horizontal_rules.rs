use regex::Regex;

pub fn remove_horizontal_rules(text: &str) -> String {
    let re = Regex::new(r"(?m)^(?:\*\*\*+|---+|___+)\s*$").unwrap();
    re.replace_all(text, "").to_string()
}
