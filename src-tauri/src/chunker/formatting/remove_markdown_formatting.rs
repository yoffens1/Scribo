use regex::Regex;

pub fn remove_markdown_formatting(text: &str) -> String {
    let mut s = text.to_string();
    
    let re_bold1 = Regex::new(r"\*\*(.+?)\*\*").unwrap();
    s = re_bold1.replace_all(&s, "$1").to_string();
    
    let re_bold2 = Regex::new(r"__(.+?)__").unwrap();
    s = re_bold2.replace_all(&s, "$1").to_string();
    
    let re_strike = Regex::new(r"~~(.+?)~~").unwrap();
    s = re_strike.replace_all(&s, "$1").to_string();
    
    let re_hi = Regex::new(r"==(.+?)==").unwrap();
    s = re_hi.replace_all(&s, "$1").to_string();
    
    let re_code = Regex::new(r"`(.+?)`").unwrap();
    s = re_code.replace_all(&s, "$1").to_string();
    
    let re_italic_under = Regex::new(r"(\b|\s|^)_([^_]+)_(\b|\s|$)").unwrap();
    s = re_italic_under.replace_all(&s, "$1$2$3").to_string();
    
    let re_italic_star = Regex::new(r"(\b|\s|^)\*([^*]+)\*(\b|\s|$)").unwrap();
    s = re_italic_star.replace_all(&s, "$1$2$3").to_string();

    s
}
