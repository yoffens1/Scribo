use crate::fragmenter::segment::Atom;
use crate::fragmenter::token::split_oversized_paragraph;
use crate::fragmenter::output::FragmentMeta;
use super::RawFragment;

pub fn pack(atoms: Vec<Atom>, max_tokens: usize, overlap_tokens: usize) -> Vec<RawFragment> {
    if atoms.is_empty() {
        return Vec::new();
    }

    let mut raw_fragments = Vec::new();
    let mut current_batch: Vec<Atom> = Vec::new();
    let mut current_tokens = 0;

    for atom in atoms {
        let pt = atom.token_count;

        if pt > max_tokens {
            flush_batch(&mut current_batch, &mut raw_fragments, &mut current_tokens);
            handle_oversized_atom(atom, max_tokens, &mut current_batch, &mut raw_fragments, &mut current_tokens);
            continue;
        }

        if current_tokens + pt > max_tokens {
            let (overlap_start, overlap_toks) = compute_overlap(&current_batch, overlap_tokens);

            let mut text_batch = Vec::with_capacity(current_batch.len());
            for a in current_batch.drain(..overlap_start) {
                text_batch.push(a.text);
            }
            for a in &current_batch {
                text_batch.push(a.text.clone());
            }

            let first_atom = current_batch.first().cloned();
            let last_atom = current_batch.last().cloned();
            let source_range = match (first_atom, last_atom) {
                (Some(f), Some(l)) => match (f.range, l.range) {
                    (Some(fr), Some(lr)) => Some(fr.start..lr.end),
                    _ => None,
                },
                _ => None,
            };

            raw_fragments.push(RawFragment {
                text: text_batch.join("\n\n"),
                meta: FragmentMeta {
                    source_range,
                    heading_path: current_batch.first().map(|a| a.heading_path.clone()).unwrap_or_default(),
                    suggested_title: current_batch.first().and_then(|a| a.heading_title.clone()),
                    is_top_level_section: current_batch.iter().any(|a| a.is_top_level),
                    ..Default::default()
                },
            });
            current_tokens = overlap_toks;
        }

        current_tokens += pt;
        current_batch.push(atom);
    }

    flush_batch(&mut current_batch, &mut raw_fragments, &mut current_tokens);
    raw_fragments
}

pub fn flush_batch(
    current_batch: &mut Vec<Atom>,
    raw_fragments: &mut Vec<RawFragment>,
    current_tokens: &mut usize,
) {
    if !current_batch.is_empty() {
        let text_batch: Vec<String> = current_batch.iter().map(|a| a.text.clone()).collect();
        let first_atom = current_batch.first().cloned();
        let last_atom = current_batch.last().cloned();
        let source_range = match (first_atom, last_atom) {
            (Some(f), Some(l)) => match (f.range, l.range) {
                (Some(fr), Some(lr)) => Some(fr.start..lr.end),
                _ => None,
            },
            _ => None,
        };

        raw_fragments.push(RawFragment {
            text: text_batch.join("\n\n"),
            meta: FragmentMeta {
                source_range,
                heading_path: current_batch.first().map(|a| a.heading_path.clone()).unwrap_or_default(),
                suggested_title: current_batch.first().and_then(|a| a.heading_title.clone()),
                is_top_level_section: current_batch.iter().any(|a| a.is_top_level),
                ..Default::default()
            },
        });
        current_batch.clear();
        *current_tokens = 0;
    }
}

pub fn compute_overlap(current_batch: &[Atom], overlap_limit: usize) -> (usize, usize) {
    let mut overlap_tokens = 0;
    let mut overlap_start = current_batch.len();
    for i in (0..current_batch.len()).rev() {
        let t = current_batch[i].token_count;
        if overlap_tokens + t <= overlap_limit {
            overlap_start = i;
            overlap_tokens += t;
        } else {
            break;
        }
    }
    (overlap_start, overlap_tokens)
}

pub fn handle_oversized_atom(
    atom: Atom,
    max_tokens: usize,
    current_batch: &mut Vec<Atom>,
    raw_fragments: &mut Vec<RawFragment>,
    current_tokens: &mut usize,
) {
    let sub_paras = split_oversized_paragraph(&atom.text, max_tokens);
    for (sub, sub_tokens) in sub_paras {
        if *current_tokens + sub_tokens > max_tokens {
            flush_batch(current_batch, raw_fragments, current_tokens);
        }

        let sub_atom = Atom {
            text: sub.clone(),
            token_count: sub_tokens,
            char_count: sub.len(),
            ..atom.clone()
        };

        if sub_tokens > max_tokens {
            raw_fragments.push(RawFragment {
                text: sub,
                meta: FragmentMeta {
                    heading_path: sub_atom.heading_path.clone(),
                    suggested_title: sub_atom.heading_title.clone(),
                    is_top_level_section: sub_atom.is_top_level,
                    ..Default::default()
                },
            });
        } else {
            *current_tokens += sub_tokens;
            current_batch.push(sub_atom);
        }
    }
}
