use std::sync::LazyLock;
use crate::chunker::types::ChunkOptions;
use crate::chunker::stages::token;

static RE_SUBHEADING: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"^#{1,6}\s").unwrap());

pub fn glue_subheadings_to_content(paragraphs: Vec<&str>) -> Vec<std::borrow::Cow<'_, str>> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < paragraphs.len() {
        let para = paragraphs[i];
        if i < paragraphs.len() - 1 && RE_SUBHEADING.is_match(para.trim_start()) {
            result.push(std::borrow::Cow::Owned(format!("{}\n\n{}", para, paragraphs[i + 1])));
            i += 2;
        } else {
            result.push(std::borrow::Cow::Borrowed(para));
            i += 1;
        }
    }
    result
}

#[derive(Clone)]
pub struct Para {
    pub text: String,
    pub tokens: usize,
}

pub fn assemble_raw_chunks(paragraphs: Vec<std::borrow::Cow<'_, str>>, options: &ChunkOptions) -> Vec<String> {
    let paras: Vec<Para> = paragraphs.into_iter().map(|text| Para {
        tokens: token::count_tokens(text.as_ref()),
        text: text.into_owned(),
    }).collect();

    let mut raw_chunks = Vec::new();
    let mut current_batch: Vec<Para> = Vec::new();
    let mut current_tokens = 0;

    for para in paras {
        let pt = para.tokens;

        if pt > options.max_tokens {
            flush_batch(&mut current_batch, &mut raw_chunks, &mut current_tokens);
            handle_oversized_para(&para.text, options, &mut current_batch, &mut raw_chunks, &mut current_tokens);
            continue;
        }

        if current_tokens + pt > options.max_tokens {
            let (overlap_start, overlap_tokens) = compute_overlap(&current_batch, options.overlap_tokens);

            let mut text_batch = Vec::with_capacity(current_batch.len());
            for p in current_batch.drain(..overlap_start) {
                text_batch.push(p.text);
            }
            for p in &current_batch {
                text_batch.push(p.text.clone());
            }
            raw_chunks.push(text_batch.join("\n\n"));
            current_tokens = overlap_tokens;
        }

        current_batch.push(para);
        current_tokens += pt;
    }

    flush_batch(&mut current_batch, &mut raw_chunks, &mut current_tokens);
    raw_chunks
}

pub fn flush_batch(current_batch: &mut Vec<Para>, raw_chunks: &mut Vec<String>, current_tokens: &mut usize) {
    if !current_batch.is_empty() {
        let text_batch: Vec<String> = current_batch.drain(..).map(|p| p.text).collect();
        raw_chunks.push(text_batch.join("\n\n"));
        *current_tokens = 0;
    }
}

pub fn compute_overlap(current_batch: &[Para], overlap_limit: usize) -> (usize, usize) {
    let mut overlap_tokens = 0;
    let mut overlap_start = current_batch.len();
    for i in (0..current_batch.len()).rev() {
        let t = current_batch[i].tokens;
        if overlap_tokens + t <= overlap_limit {
            overlap_start = i;
            overlap_tokens += t;
        } else {
            break;
        }
    }
    (overlap_start, overlap_tokens)
}

pub fn handle_oversized_para(
    para_text: &str, 
    options: &ChunkOptions, 
    current_batch: &mut Vec<Para>, 
    raw_chunks: &mut Vec<String>, 
    current_tokens: &mut usize
) {
    let sub_paras = token::split_oversized_paragraph(para_text, options.max_tokens);
    for (sub, sub_tokens) in sub_paras {
        if *current_tokens + sub_tokens > options.max_tokens {
            flush_batch(current_batch, raw_chunks, current_tokens);
        }
        
        if sub_tokens > options.max_tokens {
            raw_chunks.push(sub);
        } else {
            current_batch.push(Para { text: sub, tokens: sub_tokens });
            *current_tokens += sub_tokens;
        }
    }
}
