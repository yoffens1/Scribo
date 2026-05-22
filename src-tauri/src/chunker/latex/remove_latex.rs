use regex::Regex;

pub fn remove_latex(text: &str) -> String {
    let re_block = Regex::new(r"\$\$[\s\S]*?\$\$").unwrap();
    let mut cleaned = re_block.replace_all(text, "").to_string();
    
    let re_inline = Regex::new(r"\$[^$]+\$").unwrap();
    cleaned = re_inline.replace_all(&cleaned, "").to_string();
    
    cleaned
}
