use regex::Regex;

pub fn remove_markdown_links(text: &str) -> String {
    let re_wiki = Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").unwrap();
    let mut cleaned = re_wiki.replace_all(text, |caps: &regex::Captures| {
        if let Some(display) = caps.get(2) {
            display.as_str().to_string()
        } else {
            caps.get(1).unwrap().as_str().to_string()
        }
    }).to_string();
    
    let re_md = Regex::new(r"\[([^\]]+)\]\([^\)]+\)").unwrap();
    cleaned = re_md.replace_all(&cleaned, "$1").to_string();
    
    cleaned
}
