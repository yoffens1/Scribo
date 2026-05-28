use std::sync::Arc;
use crate::ai::LlmService;
use super::config::{FragmentConfig, Segmenter, Packer};
use super::output::{Fragment, FragmenterResult, FragmenterPair, FragmenterPairedResult};

pub struct Fragmenter {
    llm_service: Option<Arc<LlmService>>,
}

impl Fragmenter {
    pub fn new() -> Self {
        Self { llm_service: None }
    }

    pub fn with_llm(llm_service: Arc<LlmService>) -> Self {
        Self { llm_service: Some(llm_service) }
    }

    pub async fn run(&self, content: &str, config: &FragmentConfig) -> FragmenterResult {
        // 1. Frontmatter extraction
        let (metadata, remaining_content) = if config.extract_frontmatter {
            extract_yaml_frontmatter(content)
        } else {
            (None, content.to_string())
        };

        // 2. Table extraction
        let clean_flags = config.cleaner.to_flags();
        let (body_text, tables) = if clean_flags.preserve_tables {
            super::clean::tables::extract_tables(&remaining_content)
        } else {
            (remaining_content.clone(), Vec::new())
        };

        // 3. Segmenter
        let atoms = match &config.segmenter {
            Segmenter::HeadingSections { .. } => {
                super::segment::heading::segment(&body_text, config)
            }
            Segmenter::Blocks { .. } => {
                super::segment::block::segment(&body_text, config)
            }
            Segmenter::Semantic { max_chars, threshold } => {
                if let Some(llm) = &self.llm_service {
                    super::segment::semantic::segment(&body_text, *max_chars, *threshold, llm).await
                } else {
                    super::segment::block::segment(&body_text, config)
                }
            }
            Segmenter::Whole => {
                vec![super::segment::Atom::whole(&body_text)]
            }
        };

        // 4. Packer
        let mut raw_fragments = pack_atoms(atoms, config);

        // 5. Restore tables if preserved
        if clean_flags.preserve_tables && !tables.is_empty() {
            raw_fragments = super::clean::tables::restore_tables(raw_fragments, &tables, &clean_flags);
        }

        // 6. Linearize tables if enabled
        let max_tokens_limit = match config.packer {
            Packer::TokenBudget { max_tokens, .. } => max_tokens,
            _ => 256,
        };
        raw_fragments = super::clean::tables::linearize_table_fragments(raw_fragments, &clean_flags, max_tokens_limit);

        // 7. Cleaner/Transforms & meta construction
        let mut fragments = Vec::new();
        for (idx, raw_frag) in raw_fragments.into_iter().enumerate() {
            let mut cleaned_text = super::clean::transforms::apply(&raw_frag.text, &clean_flags);
            
            if config.include_heading_in_fragments {
                if let Some(heading) = &raw_frag.meta.suggested_title {
                    let clean_heading = super::clean::transforms::apply(heading, &clean_flags);
                    if !clean_heading.is_empty() && cleaned_text != clean_heading {
                        let first_line = cleaned_text.lines().next().unwrap_or("").trim();
                        if first_line != clean_heading {
                            cleaned_text = format!("{}\n{}", clean_heading, cleaned_text);
                        }
                    }
                }
            }

            let token_count = super::token::count_tokens(&cleaned_text);
            let char_count = cleaned_text.len();

            let mut meta = raw_frag.meta;
            meta.index = idx;
            meta.token_count = token_count;
            meta.char_count = char_count;

            fragments.push(Fragment {
                text: cleaned_text,
                meta,
            });
        }

        FragmenterResult { fragments, metadata }
    }

    pub fn run_sync(&self, content: &str, config: &FragmentConfig) -> FragmenterResult {
        // 1. Frontmatter extraction
        let (metadata, remaining_content) = if config.extract_frontmatter {
            extract_yaml_frontmatter(content)
        } else {
            (None, content.to_string())
        };

        // 2. Table extraction
        let clean_flags = config.cleaner.to_flags();
        let (body_text, tables) = if clean_flags.preserve_tables {
            super::clean::tables::extract_tables(&remaining_content)
        } else {
            (remaining_content.clone(), Vec::new())
        };

        // 3. Segmenter
        let atoms = match &config.segmenter {
            Segmenter::HeadingSections { .. } => {
                super::segment::heading::segment(&body_text, config)
            }
            Segmenter::Blocks { .. } => {
                super::segment::block::segment(&body_text, config)
            }
            Segmenter::Semantic { .. } => {
                // Fallback to Blocks segmenter in synchronous runs
                super::segment::block::segment(&body_text, config)
            }
            Segmenter::Whole => {
                vec![super::segment::Atom::whole(&body_text)]
            }
        };

        // 4. Packer
        let mut raw_fragments = pack_atoms(atoms, config);

        // 5. Restore tables if preserved
        if clean_flags.preserve_tables && !tables.is_empty() {
            raw_fragments = super::clean::tables::restore_tables(raw_fragments, &tables, &clean_flags);
        }

        // 6. Linearize tables if enabled
        let max_tokens_limit = match config.packer {
            Packer::TokenBudget { max_tokens, .. } => max_tokens,
            _ => 256,
        };
        raw_fragments = super::clean::tables::linearize_table_fragments(raw_fragments, &clean_flags, max_tokens_limit);

        // 7. Cleaner/Transforms & meta construction
        let mut fragments = Vec::new();
        for (idx, raw_frag) in raw_fragments.into_iter().enumerate() {
            let mut cleaned_text = super::clean::transforms::apply(&raw_frag.text, &clean_flags);
            
            if config.include_heading_in_fragments {
                if let Some(heading) = &raw_frag.meta.suggested_title {
                    let clean_heading = super::clean::transforms::apply(heading, &clean_flags);
                    if !clean_heading.is_empty() && cleaned_text != clean_heading {
                        let first_line = cleaned_text.lines().next().unwrap_or("").trim();
                        if first_line != clean_heading {
                            cleaned_text = format!("{}\n{}", clean_heading, cleaned_text);
                        }
                    }
                }
            }

            let token_count = super::token::count_tokens(&cleaned_text);
            let char_count = cleaned_text.len();

            let mut meta = raw_frag.meta;
            meta.index = idx;
            meta.token_count = token_count;
            meta.char_count = char_count;

            fragments.push(Fragment {
                text: cleaned_text,
                meta,
            });
        }

        FragmenterResult { fragments, metadata }
    }

    pub async fn run_paired(&self, content: &str, config: &FragmentConfig) -> FragmenterPairedResult {
        let structural_config = FragmentConfig {
            segmenter: config.segmenter.clone(),
            packer: config.packer.clone(),
            cleaner: super::config::CleanProfile::Raw,
            include_heading_in_fragments: config.include_heading_in_fragments,
            extract_frontmatter: config.extract_frontmatter,
        };
        let struct_result = self.run(content, &structural_config).await;

        let embed_flags = super::config::CleanProfile::Embedding.to_flags();

        let pairs: Vec<FragmenterPair> = struct_result
            .fragments
            .into_iter()
            .map(|frag| {
                let mut embedding_text = super::clean::transforms::apply(&frag.text, &embed_flags);
                if config.include_heading_in_fragments {
                    if let Some(heading) = &frag.meta.suggested_title {
                        let clean_heading = super::clean::transforms::apply(heading, &embed_flags);
                        if !clean_heading.is_empty() && embedding_text != clean_heading {
                            let first_line = embedding_text.lines().next().unwrap_or("").trim();
                            if first_line != clean_heading {
                                embedding_text = format!("{}\n{}", clean_heading, embedding_text);
                            }
                        }
                    }
                }
                FragmenterPair {
                    embedding: embedding_text,
                    generation: frag.text,
                }
            })
            .collect();

        FragmenterPairedResult {
            pairs,
            metadata: struct_result.metadata,
        }
    }

    pub fn run_paired_sync(&self, content: &str, config: &FragmentConfig) -> FragmenterPairedResult {
        let structural_config = FragmentConfig {
            segmenter: config.segmenter.clone(),
            packer: config.packer.clone(),
            cleaner: super::config::CleanProfile::Raw,
            include_heading_in_fragments: config.include_heading_in_fragments,
            extract_frontmatter: config.extract_frontmatter,
        };
        let struct_result = self.run_sync(content, &structural_config);

        let embed_flags = super::config::CleanProfile::Embedding.to_flags();

        let pairs: Vec<FragmenterPair> = struct_result
            .fragments
            .into_iter()
            .map(|frag| {
                let mut embedding_text = super::clean::transforms::apply(&frag.text, &embed_flags);
                if config.include_heading_in_fragments {
                    if let Some(heading) = &frag.meta.suggested_title {
                        let clean_heading = super::clean::transforms::apply(heading, &embed_flags);
                        if !clean_heading.is_empty() && embedding_text != clean_heading {
                            let first_line = embedding_text.lines().next().unwrap_or("").trim();
                            if first_line != clean_heading {
                                embedding_text = format!("{}\n{}", clean_heading, embedding_text);
                            }
                        }
                    }
                }
                FragmenterPair {
                    embedding: embedding_text,
                    generation: frag.text,
                }
            })
            .collect();

        FragmenterPairedResult {
            pairs,
            metadata: struct_result.metadata,
        }
    }
}

fn extract_yaml_frontmatter(content: &str) -> (Option<serde_json::Map<String, serde_json::Value>>, String) {
    if let Some(stripped) = content.strip_prefix("---\n") {
        if let Some(end_idx) = stripped.find("\n---\n") {
            let yaml_text = &stripped[..end_idx];
            let remaining = stripped[end_idx + 5..].to_string();
            
            let metadata = match serde_yaml::from_str::<serde_json::Map<String, serde_json::Value>>(yaml_text) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Failed to parse YAML frontmatter: {}", e);
                    serde_json::Map::new()
                }
            };
            
            return (if metadata.is_empty() { None } else { Some(metadata) }, remaining);
        }
    }
    (None, content.to_string())
}

fn pack_atoms(atoms: Vec<super::segment::Atom>, config: &FragmentConfig) -> Vec<super::pack::RawFragment> {
    let is_heading_sections = matches!(&config.segmenter, Segmenter::HeadingSections { .. });
    
    match &config.packer {
        Packer::TokenBudget { max_tokens, overlap_tokens } => {
            if is_heading_sections {
                let mut raw_fragments = Vec::new();
                let mut current_section_atoms = Vec::new();
                let mut last_path = None;
                for atom in atoms {
                    let path = atom.heading_path.clone();
                    if last_path.is_some() && Some(&path) != last_path.as_ref() {
                        let batch = std::mem::take(&mut current_section_atoms);
                        raw_fragments.extend(super::pack::token_budget::pack(batch, *max_tokens, *overlap_tokens));
                    }
                    last_path = Some(path);
                    current_section_atoms.push(atom);
                }
                if !current_section_atoms.is_empty() {
                    raw_fragments.extend(super::pack::token_budget::pack(current_section_atoms, *max_tokens, *overlap_tokens));
                }
                raw_fragments
            } else {
                super::pack::token_budget::pack(atoms, *max_tokens, *overlap_tokens)
            }
        }
        Packer::CharBudget { max_chars } => {
            if is_heading_sections {
                let mut raw_fragments = Vec::new();
                let mut current_section_atoms = Vec::new();
                let mut last_path = None;
                for atom in atoms {
                    let path = atom.heading_path.clone();
                    if last_path.is_some() && Some(&path) != last_path.as_ref() {
                        let batch = std::mem::take(&mut current_section_atoms);
                        raw_fragments.extend(super::pack::char_budget::pack(batch, *max_chars));
                    }
                    last_path = Some(path);
                    current_section_atoms.push(atom);
                }
                if !current_section_atoms.is_empty() {
                    raw_fragments.extend(super::pack::char_budget::pack(current_section_atoms, *max_chars));
                }
                raw_fragments
            } else {
                super::pack::char_budget::pack(atoms, *max_chars)
            }
        }
        Packer::Passthrough => {
            super::pack::passthrough::pack(atoms)
        }
    }
}
