use std::ops::Range;

pub mod heading;
pub mod block;
pub mod semantic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomKind {
    Heading { level: u8 },
    Paragraph,
    List,
    CodeBlock,
    Table,
    Other,
}

#[derive(Debug, Clone)]
pub struct Atom {
    pub text: String,
    pub kind: AtomKind,
    pub range: Option<Range<usize>>,
    pub heading_path: Vec<String>,
    pub heading_title: Option<String>,
    pub is_top_level: bool,
    pub token_count: usize,
    pub char_count: usize,
}

impl Atom {
    pub fn whole(content: &str) -> Self {
        Self {
            text: content.to_string(),
            kind: AtomKind::Other,
            range: Some(0..content.len()),
            heading_path: Vec::new(),
            heading_title: None,
            is_top_level: false,
            token_count: crate::fragmenter::token::count_tokens(content),
            char_count: content.len(),
        }
    }
}
