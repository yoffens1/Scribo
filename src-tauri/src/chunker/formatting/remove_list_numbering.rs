use regex::Regex;

pub fn remove_list_numbering(text: &str) -> String {
    let re = Regex::new(r"(?m)^[\s]*\d+\.\s+").unwrap();
    re.replace_all(text, "").to_string()
}
