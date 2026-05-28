use pulldown_cmark::{Event, Tag, HeadingLevel, Options, Parser};
use crate::domain::distribute::{TopicChunk, RawBlock};
use crate::ai::LlmService;

#[allow(async_fn_in_trait)]
pub trait Chunker: Send + Sync {
    async fn chunk(&self, content: &str, llm_service: &LlmService) -> Vec<TopicChunk>;
}

pub struct RuleChunker {
    pub max_chars: usize,
}

impl RuleChunker {
    pub fn new(max_chars: usize) -> Self {
        Self { max_chars }
    }
}

impl Chunker for RuleChunker {
    async fn chunk(&self, content: &str, _llm_service: &LlmService) -> Vec<TopicChunk> {
        split_into_topics(content, self.max_chars)
    }
}

pub struct SemanticChunker {
    pub max_chars: usize,
    pub threshold: f32,
}

impl SemanticChunker {
    pub fn new(max_chars: usize, threshold: f32) -> Self {
        Self { max_chars, threshold }
    }
}

use crate::utils::cosine_similarity;

impl Chunker for SemanticChunker {
    async fn chunk(&self, content: &str, llm_service: &LlmService) -> Vec<TopicChunk> {
        let raw_blocks = parse_raw_blocks(content);
        if raw_blocks.is_empty() {
            return Vec::new();
        }

        let texts: Vec<String> = raw_blocks.iter().map(|b| b.text.clone()).collect();
        let embeddings = match llm_service.generate_embeddings(texts.clone()).await {
            Ok(embs) => embs,
            Err(_) => vec![vec![0.0f32; 1536]; texts.len()],
        };

        let mut chunks = Vec::new();
        let mut current_chunk_blocks = vec![raw_blocks[0].clone()];

        for i in 1..raw_blocks.len() {
            let sim = if i < embeddings.len() && i - 1 < embeddings.len() {
                cosine_similarity(&embeddings[i - 1], &embeddings[i])
            } else {
                0.0
            };

            let total_len: usize = current_chunk_blocks.iter().map(|b| b.text.len()).sum();
            let block_len = raw_blocks[i].text.len();

            if sim < self.threshold || (total_len + block_len > self.max_chars) {
                let text = current_chunk_blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n\n");
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

                chunks.push(TopicChunk { text, suggested_title });
                current_chunk_blocks.clear();
            }

            current_chunk_blocks.push(raw_blocks[i].clone());
        }

        if !current_chunk_blocks.is_empty() {
            let text = current_chunk_blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n\n");
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
            chunks.push(TopicChunk { text, suggested_title });
        }

        chunks
    }
}

pub fn parse_raw_blocks(content: &str) -> Vec<RawBlock> {
    let options = Options::all();
    let parser = Parser::new_ext(content, options);
    
    let mut raw_blocks = Vec::new();
    let mut depth = 0;
    let mut current_block_start = None;
    let mut current_block_is_h1_h2 = false;
    let mut current_block_text_accumulator = String::new();
    let mut in_heading = false;
    
    for (event, range) in parser.into_offset_iter() {
        if depth == 0 {
            match &event {
                Event::Start(tag) => {
                    current_block_start = Some(range.start);
                    depth += 1;
                    if let Tag::Heading { level, .. } = tag {
                        if *level == HeadingLevel::H1 || *level == HeadingLevel::H2 {
                            current_block_is_h1_h2 = true;
                        }
                        in_heading = true;
                        current_block_text_accumulator.clear();
                    }
                }
                _ => {
                    let block_text = content[range.clone()].to_string();
                    if !block_text.trim().is_empty() {
                        raw_blocks.push(RawBlock {
                            range,
                            text: block_text,
                            is_heading_h1_h2: false,
                            heading_title: None,
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
                                raw_blocks.push(RawBlock {
                                    range: block_range,
                                    text: block_text,
                                    is_heading_h1_h2: current_block_is_h1_h2,
                                    heading_title,
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
    
    raw_blocks
}

pub fn split_into_topics(content: &str, max_chars: usize) -> Vec<TopicChunk> {
    let raw_blocks = parse_raw_blocks(content);
    if raw_blocks.is_empty() {
        return Vec::new();
    }
    
    let mut chunks = Vec::new();
    let mut current_chunk_blocks: Vec<RawBlock> = Vec::new();
    let mut current_chunk_len = 0;
    
    for block in raw_blocks {
        let block_len = block.text.len();
        
        let should_flush = !current_chunk_blocks.is_empty() && (
            block.is_heading_h1_h2 || (current_chunk_len + block_len > max_chars)
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
            
            chunks.push(TopicChunk {
                text: chunk_text,
                suggested_title,
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
        
        chunks.push(TopicChunk {
            text: chunk_text,
            suggested_title,
        });
    }
    
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_into_topics() {
        let content = "\
# Math Note
This is some content.

## Section 2
And some more content here.
- Item 1
- Item 2
";
        let chunks = split_into_topics(content, 1000);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].suggested_title, "Math Note");
        assert!(chunks[0].text.contains("This is some content."));
        assert_eq!(chunks[1].suggested_title, "Section 2");
        assert!(chunks[1].text.contains("Item 2"));
    }
}
