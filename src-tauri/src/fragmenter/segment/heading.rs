use std::sync::LazyLock;
use std::ops::Range;
use regex::Regex;
use crate::fragmenter::config::FragmentConfig;
use crate::fragmenter::segment::{Atom, AtomKind};
use crate::fragmenter::token::count_tokens;

static RE_PARA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n").unwrap());
static RE_SUBHEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}\s").unwrap());

static RE_HEADING_EXACT: LazyLock<[Regex; 6]> = LazyLock::new(|| {
    [
        Regex::new(r"(?m)^#\s").unwrap(),
        Regex::new(r"(?m)^##\s").unwrap(),
        Regex::new(r"(?m)^###\s").unwrap(),
        Regex::new(r"(?m)^####\s").unwrap(),
        Regex::new(r"(?m)^#####\s").unwrap(),
        Regex::new(r"(?m)^######\s").unwrap(),
    ]
});
static RE_ANY_HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s").unwrap());

static RE_SUB_LEVELS: LazyLock<[Regex; 6]> = LazyLock::new(|| [
    Regex::new(r"(?m)^#\s").unwrap(),
    Regex::new(r"(?m)^##\s").unwrap(),
    Regex::new(r"(?m)^###\s").unwrap(),
    Regex::new(r"(?m)^####\s").unwrap(),
    Regex::new(r"(?m)^#####\s").unwrap(),
    Regex::new(r"(?m)^######\s").unwrap(),
]);

pub fn split_by_heading_pattern<'a>(text: &'a str, heading_regex: &Regex) -> Vec<(&'a str, Range<usize>)> {
    let mut sections = Vec::new();
    let mut last_idx = 0;

    for mat in heading_regex.find_iter(text) {
        if mat.start() > last_idx {
            let section = &text[last_idx..mat.start()];
            let mut end_trim = mat.start();
            if section.ends_with('\n') {
                end_trim -= 1;
            }
            if text[last_idx..end_trim].ends_with('\r') {
                end_trim -= 1;
            }
            sections.push((&text[last_idx..end_trim], last_idx..end_trim));
        }
        last_idx = mat.start();
    }

    if last_idx < text.len() {
        sections.push((&text[last_idx..], last_idx..text.len()));
    }

    sections
}

pub fn split_by_headings(text: &str, level: usize) -> Vec<(&str, Range<usize>)> {
    let heading_regex = if (1..=6).contains(&level) {
        &RE_HEADING_EXACT[level - 1]
    } else {
        &RE_ANY_HEADING
    };

    split_by_heading_pattern(text, heading_regex)
}

pub fn extract_section_heading(section: &str, level: usize) -> Option<String> {
    let first_line = section.trim_start().lines().next()?.trim();
    let idx = level.saturating_sub(1).min(5);
    let re = &RE_HEADING_EXACT[idx];
    if re.is_match(first_line) {
        Some(first_line.to_string())
    } else {
        None
    }
}

pub fn segment(content: &str, config: &FragmentConfig) -> Vec<Atom> {
    let level = match &config.segmenter {
        crate::fragmenter::config::Segmenter::HeadingSections { level, .. } => *level,
        _ => 2,
    };
    let separate_sub_headings = match &config.segmenter {
        crate::fragmenter::config::Segmenter::HeadingSections { separate_sub_headings, .. } => *separate_sub_headings,
        _ => false,
    };
    let keep_subheading_with_content = match &config.segmenter {
        crate::fragmenter::config::Segmenter::HeadingSections { keep_subheading_with_content, .. } => *keep_subheading_with_content,
        _ => true,
    };

    let sub_level = level + 1;
    let sub_regex = if sub_level <= 6 {
        Some(&RE_SUB_LEVELS[sub_level - 1])
    } else {
        None
    };

    let sections = split_by_headings(content, level);
    let mut atoms = Vec::new();

    for (section, _section_range) in sections {
        let section_heading = extract_section_heading(section, level);
        let heading_title = section_heading.clone().map(|h| h.replace("#", "").trim().to_string());
        
        let paragraphs: Vec<(&str, Range<usize>)> = RE_PARA
            .split(section)
            .filter(|p| !p.trim().is_empty())
            .map(|p| {
                let offset = (p.as_ptr() as usize).saturating_sub(content.as_ptr() as usize);
                (p, offset..(offset + p.len()))
            })
            .collect();

        let mut i = 0;
        let mut section_atoms = Vec::new();
        while i < paragraphs.len() {
            let (para_text, para_range) = paragraphs[i].clone();

            if i < paragraphs.len() - 1 && keep_subheading_with_content && RE_SUBHEADING.is_match(para_text.trim_start()) {
                let (_, next_range) = paragraphs[i + 1].clone();
                let glued_text = content[para_range.start..next_range.end].to_string();
                
                section_atoms.push(Atom {
                    text: glued_text.clone(),
                    kind: AtomKind::Paragraph,
                    range: Some(para_range.start..next_range.end),
                    heading_path: section_heading.clone().into_iter().collect(),
                    heading_title: heading_title.clone(),
                    is_top_level: false,
                    token_count: count_tokens(&glued_text),
                    char_count: glued_text.len(),
                });
                i += 2;
            } else {
                let is_heading = RE_SUBHEADING.is_match(para_text.trim_start());
                let kind = if is_heading {
                    let level = para_text.trim_start().chars().take_while(|&c| c == '#').count() as u8;
                    AtomKind::Heading { level }
                } else {
                    AtomKind::Paragraph
                };

                section_atoms.push(Atom {
                    text: para_text.to_string(),
                    kind,
                    range: Some(para_range),
                    heading_path: section_heading.clone().into_iter().collect(),
                    heading_title: heading_title.clone(),
                    is_top_level: false,
                    token_count: count_tokens(para_text),
                    char_count: para_text.len(),
                });
                i += 1;
            }
        }

        if separate_sub_headings {
            if let Some(re) = sub_regex {
                for atom in section_atoms {
                    if re.is_match(&atom.text) {
                        let parts = split_by_heading_pattern(&atom.text, re);
                        for (part_text, part_range) in parts {
                            let part_start = atom.range.as_ref().map(|r| r.start).unwrap_or(0) + part_range.start;
                            let part_end = atom.range.as_ref().map(|r| r.start).unwrap_or(0) + part_range.end;
                            let is_heading = RE_SUBHEADING.is_match(part_text.trim_start());
                            let kind = if is_heading {
                                let lvl = part_text.trim_start().chars().take_while(|&c| c == '#').count() as u8;
                                AtomKind::Heading { level: lvl }
                            } else {
                                AtomKind::Paragraph
                            };
                            atoms.push(Atom {
                                text: part_text.to_string(),
                                kind,
                                range: Some(part_start..part_end),
                                heading_path: atom.heading_path.clone(),
                                heading_title: atom.heading_title.clone(),
                                is_top_level: false,
                                token_count: count_tokens(part_text),
                                char_count: part_text.len(),
                            });
                        }
                    } else {
                        atoms.push(atom);
                    }
                }
            } else {
                atoms.extend(section_atoms);
            }
        } else {
            atoms.extend(section_atoms);
        }
    }

    atoms
}
