//! # Fragmenter Pipeline
//!
//! [`Fragmenter`] is the central orchestrator that drives every stage of the pipeline.
//! It holds an optional [`LlmService`] reference needed only for [`Segmenter::Semantic`].
//!
//! ## Pipeline stages (executed in order)
//!
//! 1. **Frontmatter extraction** — strips and parses YAML `---...---` blocks.
//! 2. **Table extraction** — lifts pipe-tables out of the body and replaces them with
//!    placeholders so they don't interfere with line-level segmenters.
//! 3. **Segmentation** — produces a `Vec<Atom>` via the configured [`Segmenter`].
//! 4. **Packing** — merges atoms into `Vec<RawFragment>` via the configured [`Packer`].
//! 5. **Table restore** — splices the original tables back at their placeholder positions.
//! 6. **Table linearisation** — optionally converts table syntax to prose for embeddings.
//! 7. **Cleaning & meta** — applies [`CleanProfile`] transformations and computes token/char counts.
//!
//! ## Async vs Sync
//!
//! - [`run`](Fragmenter::run) / [`run_paired`](Fragmenter::run_paired) — async, required for [`Segmenter::Semantic`].
//! - [`run_sync`](Fragmenter::run_sync) / [`run_paired_sync`](Fragmenter::run_paired_sync) — sync, falls back to `Blocks`
//!   segmentation when `Semantic` is requested (no LLM call possible in sync context).

use std::sync::Arc;
use crate::ai::LlmService;
use super::config::{FragmentConfig, Segmenter, Packer};
use super::output::{Fragment, FragmenterResult, FragmenterPair, FragmenterPairedResult};

/// The main fragmenter struct. Create with [`Fragmenter::new`] for rule-based pipelines,
/// or [`Fragmenter::with_llm`] when semantic segmentation is required.
pub struct Fragmenter {
    /// LLM service used exclusively by [`Segmenter::Semantic`] to generate block embeddings.
    llm_service: Option<Arc<LlmService>>,
}

impl Fragmenter {
    /// Creates a fragmenter without LLM support.
    /// [`Segmenter::Semantic`] will fall back to `Blocks` if used with this variant.
    pub fn new() -> Self {
        Self { llm_service: None }
    }

    /// Creates a fragmenter with an LLM service, enabling semantic segmentation.
    pub fn with_llm(llm_service: Arc<LlmService>) -> Self {
        Self { llm_service: Some(llm_service) }
    }

    /// Runs the full pipeline asynchronously.
    /// Supports all segmenters including [`Segmenter::Semantic`].
    pub async fn run(&self, content: &str, config: &FragmentConfig) -> FragmenterResult {
        // 1. Strip and parse YAML frontmatter (---...---)
        let (metadata, remaining_content) = if config.extract_frontmatter {
            extract_yaml_frontmatter(content)
        } else {
            (None, content.to_string())
        };

        // 2. Extract pipe-tables from the body so segmenters work on clean line-oriented text.
        //    Tables are returned as (placeholder_id, TableInfo) pairs and restored in step 5.
        let clean_flags = config.cleaner.to_flags();
        let (body_text, tables) = if clean_flags.preserve_tables {
            super::clean::tables::extract_tables(&remaining_content)
        } else {
            (remaining_content.clone(), Vec::new())
        };

        // 3. Segment the body into Atoms according to the configured Segmenter.
        let atoms = match &config.segmenter {
            Segmenter::HeadingSections { .. } => {
                super::segment::heading::segment(&body_text, config)
            }
            Segmenter::Blocks { .. } => {
                super::segment::block::segment(&body_text, config)
            }
            Segmenter::Semantic { max_chars, threshold } => {
                if let Some(llm) = &self.llm_service {
                    // Uses embeddings to merge similar consecutive blocks.
                    super::segment::semantic::segment(&body_text, *max_chars, *threshold, llm).await
                } else {
                    // No LLM available — degrade gracefully to block segmentation.
                    super::segment::block::segment(&body_text, config)
                }
            }
            Segmenter::Whole => {
                vec![super::segment::Atom::whole(&body_text)]
            }
        };

        // 4. Pack atoms into RawFragments according to the configured Packer.
        let mut raw_fragments = pack_atoms(atoms, config);

        // 5. Re-inject the extracted tables at their original positions.
        if clean_flags.preserve_tables && !tables.is_empty() {
            raw_fragments = super::pack::tables::restore_tables(raw_fragments, &tables, &clean_flags);
        }

        // 7. Apply the CleanProfile to every fragment and compute metadata.
        let mut fragments = Vec::new();
        for (idx, raw_frag) in raw_fragments.into_iter().enumerate() {
            let mut cleaned_text = super::clean::apply(&raw_frag.text, &clean_flags);

            // Optionally prepend the section heading so the embedding model has full context.
            if config.include_heading_in_fragments {
                if let Some(heading) = &raw_frag.meta.suggested_title {
                    let clean_heading = super::clean::apply(heading, &clean_flags);
                    let first_line_norm = cleaned_text.lines().next().unwrap_or("").trim().to_lowercase();
                    let heading_norm = clean_heading.trim().to_lowercase();
                    if !heading_norm.is_empty() && first_line_norm != heading_norm {
                        cleaned_text = format!("{}\n{}", clean_heading, cleaned_text);
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

    /// Runs the pipeline synchronously.
    /// [`Segmenter::Semantic`] falls back to `Blocks` because embedding generation is async-only.
    pub fn run_sync(&self, content: &str, config: &FragmentConfig) -> FragmenterResult {
        // 1. Frontmatter
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

        // 3. Segmentation — Semantic degrades to Blocks in sync context
        let atoms = match &config.segmenter {
            Segmenter::HeadingSections { .. } => {
                super::segment::heading::segment(&body_text, config)
            }
            Segmenter::Blocks { .. } => {
                super::segment::block::segment(&body_text, config)
            }
            Segmenter::Semantic { .. } => {
                // Fallback: no async executor available, use block segmentation
                super::segment::block::segment(&body_text, config)
            }
            Segmenter::Whole => {
                vec![super::segment::Atom::whole(&body_text)]
            }
        };

        // 4–7. Identical to the async path
        let mut raw_fragments = pack_atoms(atoms, config);

        if clean_flags.preserve_tables && !tables.is_empty() {
            raw_fragments = super::pack::tables::restore_tables(raw_fragments, &tables, &clean_flags);
        }

        let mut fragments = Vec::new();
        for (idx, raw_frag) in raw_fragments.into_iter().enumerate() {
            let mut cleaned_text = super::clean::apply(&raw_frag.text, &clean_flags);

            if config.include_heading_in_fragments {
                if let Some(heading) = &raw_frag.meta.suggested_title {
                    let clean_heading = super::clean::apply(heading, &clean_flags);
                    let first_line_norm = cleaned_text.lines().next().unwrap_or("").trim().to_lowercase();
                    let heading_norm = clean_heading.trim().to_lowercase();
                    if !heading_norm.is_empty() && first_line_norm != heading_norm {
                        cleaned_text = format!("{}\n{}", clean_heading, cleaned_text);
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

    /// Runs the pipeline in "paired" mode asynchronously.
    /// Segments once with `Raw` cleaning, then derives both the embedding and generation
    /// text from the same raw fragment — guaranteeing alignment between the two.
    pub async fn run_paired(&self, content: &str, config: &FragmentConfig) -> FragmenterPairedResult {
        // First pass with Raw cleaning to preserve the original structure
        let structural_config = FragmentConfig {
            segmenter: config.segmenter.clone(),
            packer: config.packer.clone(),
            cleaner: super::config::CleanProfile::Raw,
            include_heading_in_fragments: config.include_heading_in_fragments,
            extract_frontmatter: config.extract_frontmatter,
        };
        let struct_result = self.run(content, &structural_config).await;

        let embed_flags = super::config::CleanProfile::Embedding.to_flags();

        // Apply Embedding cleaning to every raw fragment in-memory (no re-segmentation).
        let pairs: Vec<FragmenterPair> = struct_result
            .fragments
            .into_iter()
            .map(|frag| {
                let mut embedding_text = super::clean::apply(&frag.text, &embed_flags);
                if config.include_heading_in_fragments {
                    if let Some(heading) = &frag.meta.suggested_title {
                        let clean_heading = super::clean::apply(heading, &embed_flags);
                        let first_line_norm = embedding_text.lines().next().unwrap_or("").trim().to_lowercase();
                        let heading_norm = clean_heading.trim().to_lowercase();
                        if !heading_norm.is_empty() && first_line_norm != heading_norm {
                            embedding_text = format!("{}\n{}", clean_heading, embedding_text);
                        }
                    }
                }
                FragmenterPair {
                    embedding: embedding_text,
                    generation: frag.text, // raw text becomes the generation surface
                }
            })
            .collect();

        FragmenterPairedResult {
            pairs,
            metadata: struct_result.metadata,
        }
    }

    /// Synchronous variant of [`run_paired`](Self::run_paired).
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
                let mut embedding_text = super::clean::apply(&frag.text, &embed_flags);
                if config.include_heading_in_fragments {
                    if let Some(heading) = &frag.meta.suggested_title {
                        let clean_heading = super::clean::apply(heading, &embed_flags);
                        let first_line_norm = embedding_text.lines().next().unwrap_or("").trim().to_lowercase();
                        let heading_norm = clean_heading.trim().to_lowercase();
                        if !heading_norm.is_empty() && first_line_norm != heading_norm {
                            embedding_text = format!("{}\n{}", clean_heading, embedding_text);
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

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Extracts and parses a YAML frontmatter block delimited by `---\n...\n---\n`.
/// Returns `(Some(metadata_map), remaining_body)` on success, or `(None, original_content)` when
/// no frontmatter is found or parsing fails.
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

/// Dispatches atoms to the appropriate packer.
///
/// When the segmenter is `HeadingSections`, atoms are first grouped by their `heading_path`
/// before being passed to the packer — this ensures that fragment boundaries always align
/// with section boundaries, preventing cross-section token windows.
fn pack_atoms(atoms: Vec<super::segment::Atom>, config: &FragmentConfig) -> Vec<super::pack::RawFragment> {
    let is_heading_sections = matches!(&config.segmenter, Segmenter::HeadingSections { .. });

    match &config.packer {
        Packer::TokenBudget { max_tokens, overlap_tokens } => {
            if is_heading_sections {
                // Pack within each heading section independently to avoid cross-section windows.
                let mut raw_fragments = Vec::new();
                let mut current_section_atoms = Vec::new();
                let mut last_path = None;
                for atom in atoms {
                    let path = atom.heading_path.clone();
                    if last_path.is_some() && Some(&path) != last_path.as_ref() {
                        // Section boundary — flush the current batch.
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
