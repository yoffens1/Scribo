use pulldown_cmark::{Event, Tag, HeadingLevel, Options, Parser};
use crate::fragmenter::config::FragmentConfig;
use crate::fragmenter::segment::{Atom, AtomKind};
use crate::fragmenter::token::count_tokens;

pub fn segment(content: &str, _config: &FragmentConfig) -> Vec<Atom> {
    let options = Options::all();
    let parser = Parser::new_ext(content, options);
    
    let mut atoms = Vec::new();
    let mut depth = 0;
    let mut current_block_start = None;
    let mut current_block_is_h1_h2 = false;
    let mut current_block_text_accumulator = String::new();
    let mut in_heading = false;
    let mut current_heading_level = 1u8;
    
    for (event, range) in parser.into_offset_iter() {
        if depth == 0 {
            match &event {
                Event::Start(tag) => {
                    current_block_start = Some(range.start);
                    depth += 1;
                    if let Tag::Heading { level, .. } = tag {
                        let level_val = match level {
                            HeadingLevel::H1 => 1,
                            HeadingLevel::H2 => 2,
                            HeadingLevel::H3 => 3,
                            HeadingLevel::H4 => 4,
                            HeadingLevel::H5 => 5,
                            HeadingLevel::H6 => 6,
                        };
                        current_heading_level = level_val;
                        if level_val <= 2 {
                            current_block_is_h1_h2 = true;
                        }
                        in_heading = true;
                        current_block_text_accumulator.clear();
                    }
                }
                _ => {
                    let block_text = content[range.clone()].to_string();
                    if !block_text.trim().is_empty() {
                        let tokens = count_tokens(&block_text);
                        atoms.push(Atom {
                            text: block_text.clone(),
                            kind: AtomKind::Paragraph,
                            range: Some(range),
                            heading_path: Vec::new(),
                            heading_title: None,
                            is_top_level: false,
                            token_count: tokens,
                            char_count: block_text.len(),
                        });
                    }
                }
            }
        } else {
            match &event {
                Event::Start(_) => {
                    depth += 1;
                }
                Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(start) = current_block_start {
                            let block_range = start..range.end;
                            let block_text = content[block_range.clone()].to_string();
                            
                            let heading_title = if in_heading {
                                Some(current_block_text_accumulator.trim().to_string())
                            } else {
                                None
                            };

                            if !block_text.trim().is_empty() {
                                let kind = if in_heading {
                                    AtomKind::Heading { level: current_heading_level }
                                } else if block_text.starts_with('|') || block_text.contains("\n|") {
                                    AtomKind::Table
                                } else {
                                    AtomKind::Paragraph
                                };

                                let tokens = count_tokens(&block_text);
                                atoms.push(Atom {
                                    text: block_text.clone(),
                                    kind,
                                    range: Some(block_range),
                                    heading_path: Vec::new(),
                                    heading_title,
                                    is_top_level: current_block_is_h1_h2,
                                    token_count: tokens,
                                    char_count: block_text.len(),
                                });
                            }
                            
                            current_block_start = None;
                            current_block_is_h1_h2 = false;
                            current_block_text_accumulator.clear();
                            in_heading = false;
                        }
                    }
                }
                Event::Text(txt) | Event::Code(txt) => {
                    if in_heading {
                        current_block_text_accumulator.push_str(txt);
                    }
                }
                _ => {}
            }
        }
    }
    
    atoms
}
