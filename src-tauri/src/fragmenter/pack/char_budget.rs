use crate::fragmenter::segment::Atom;
use crate::fragmenter::output::FragmentMeta;
use super::RawFragment;

pub fn pack(atoms: Vec<Atom>, max_chars: usize) -> Vec<RawFragment> {
    if atoms.is_empty() {
        return Vec::new();
    }
    
    let mut chunks = Vec::new();
    let mut current_chunk_blocks: Vec<Atom> = Vec::new();
    let mut current_chunk_len = 0;
    
    for block in atoms {
        let block_len = block.text.len();
        
        let should_flush = !current_chunk_blocks.is_empty() && (
            block.is_top_level || (current_chunk_len + block_len > max_chars)
        );
        
        if should_flush {
            let chunk_text = current_chunk_blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n\n");
            
            let suggested_title = current_chunk_blocks.iter()
                .find_map(|b| b.heading_title.clone())
                .unwrap_or_else(|| {
                    let first_line = current_chunk_blocks[0].text.lines().next().unwrap_or("Untitled").trim();
                    let clean = first_line.replace("#", "").trim().to_string();
                    if clean.chars().count() > 40 {
                        let truncated: String = clean.chars().take(40).collect();
                        format!("{}...", truncated)
                    } else {
                        clean
                    }
                });
            
            let first_atom = current_chunk_blocks.first().cloned();
            let last_atom = current_chunk_blocks.last().cloned();
            let source_range = match (first_atom, last_atom) {
                (Some(f), Some(l)) => match (f.range, l.range) {
                    (Some(fr), Some(lr)) => Some(fr.start..lr.end),
                    _ => None,
                },
                _ => None,
            };

            chunks.push(RawFragment {
                text: chunk_text,
                meta: FragmentMeta {
                    source_range,
                    heading_path: current_chunk_blocks.first().map(|b| b.heading_path.clone()).unwrap_or_default(),
                    suggested_title: Some(suggested_title),
                    is_top_level_section: current_chunk_blocks.iter().any(|b| b.is_top_level),
                    ..Default::default()
                },
            });
            
            current_chunk_blocks.clear();
            current_chunk_len = 0;
        }
        
        current_chunk_len += block_len;
        current_chunk_blocks.push(block);
    }
    
    if !current_chunk_blocks.is_empty() {
        let chunk_text = current_chunk_blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n\n");
        let suggested_title = current_chunk_blocks.iter()
            .find_map(|b| b.heading_title.clone())
            .unwrap_or_else(|| {
                let first_line = current_chunk_blocks[0].text.lines().next().unwrap_or("Untitled").trim();
                let clean = first_line.replace("#", "").trim().to_string();
                if clean.chars().count() > 40 {
                    let truncated: String = clean.chars().take(40).collect();
                    format!("{}...", truncated)
                } else {
                    clean
                }
            });
        
        let first_atom = current_chunk_blocks.first().cloned();
        let last_atom = current_chunk_blocks.last().cloned();
        let source_range = match (first_atom, last_atom) {
            (Some(f), Some(l)) => match (f.range, l.range) {
                (Some(fr), Some(lr)) => Some(fr.start..lr.end),
                _ => None,
            },
            _ => None,
        };

        chunks.push(RawFragment {
            text: chunk_text,
            meta: FragmentMeta {
                source_range,
                heading_path: current_chunk_blocks.first().map(|b| b.heading_path.clone()).unwrap_or_default(),
                suggested_title: Some(suggested_title),
                is_top_level_section: current_chunk_blocks.iter().any(|b| b.is_top_level),
                ..Default::default()
            },
        });
    }
    
    chunks
}
