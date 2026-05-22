pub mod extract;
pub mod formatting;
pub mod latex;
pub mod table;
pub mod token;
pub mod types;

pub use types::{ChunkOptions, ChunkerPair, ChunkerResult};

pub struct Chunker {
    options: ChunkOptions,
}

impl Chunker {
    pub fn new(options: ChunkOptions) -> Self {
        Self { options }
    }

    pub fn chunk_paired(&self, content: String) -> ChunkerResult {
        // Single structural split — same options for both paths
        let (struct_chunks, metadata) = self.run_pipeline(&content, &self.options.merge_with_structural());

        // Clean each raw chunk with embedding and generation options
        let embed_opts = self.options.merge_with_embedding();
        let gen_opts = self.options.merge_with_generation();

        let mut pairs = Vec::new();
        for raw in struct_chunks {
            let embedding = self.clean_chunk(&raw, &embed_opts);
            let generation = self.clean_chunk(&raw, &gen_opts);
            pairs.push(ChunkerPair { embedding, generation });
        }

        ChunkerResult { pairs, metadata }
    }

    pub fn chunk_for_embedding(&self, content: &str) -> Vec<String> {
        let (chunks, _) = self.run_pipeline(content, &self.options.merge_with_embedding());
        chunks
    }

    pub fn chunk_for_generation(&self, content: &str) -> Vec<String> {
        let (chunks, _) = self.run_pipeline(content, &self.options.merge_with_generation());
        chunks
    }

    fn run_pipeline(
        &self,
        content: &str,
        options: &ChunkOptions,
    ) -> (Vec<String>, Option<std::collections::HashMap<String, serde_json::Value>>) {
        let (metadata, remaining_content) = extract::extract_yaml_frontmatter(content);

        let chunks = if options.chunk_by_headings {
            self.chunk_by_heading_sections(&remaining_content, options)
        } else {
            self.process_section(&remaining_content, options, None)
        };

        (chunks, metadata)
    }

    fn chunk_by_heading_sections(&self, content: &str, options: &ChunkOptions) -> Vec<String> {
        let sections = extract::split_by_headings(content, options.heading_level);
        let mut all_chunks = Vec::new();

        for section in sections {
            let section_heading = self.extract_section_heading(&section, options);
            let target_heading = if options.include_heading_in_chunks {
                section_heading
            } else {
                None
            };
            let section_chunks = self.process_section(&section, options, target_heading.as_deref());
            all_chunks.extend(section_chunks);
        }

        all_chunks
    }

    fn extract_section_heading(&self, section: &str, options: &ChunkOptions) -> Option<String> {
        let first_line = section.trim_start().lines().next()?.trim();
        let pattern = format!(r"^#{{{},6}}\s", options.heading_level);
        let re = regex::Regex::new(&pattern).unwrap();
        if re.is_match(first_line) {
            Some(first_line.to_string())
        } else {
            None
        }
    }

    fn process_section(&self, text: &str, options: &ChunkOptions, section_heading: Option<&str>) -> Vec<String> {
        // 1. Extract tables
        let (body_text, tables) = if options.preserve_tables {
            table::extract_tables(text)
        } else {
            (text.to_string(), Vec::new())
        };

        // 2. Split into paragraph blocks
        let re_para = regex::Regex::new(r"\n\s*\n").unwrap();
        let mut paragraphs: Vec<String> = re_para
            .split(&body_text)
            .filter(|p| !p.trim().is_empty())
            .map(|p| p.to_string())
            .collect();

        // 3. Glue subheadings
        if options.keep_subheading_with_content {
            paragraphs = self.glue_subheadings_to_content(paragraphs);
        }

        // 4. Assemble raw chunks
        let raw_chunks = self.assemble_raw_chunks(paragraphs, options);

        // 5. Restore tables
        let mut merged_chunks = self.restore_tables(raw_chunks, &tables, options);

        // 5.5 Split by subheadings
        if options.separate_sub_headings {
            merged_chunks = self.split_chunks_by_sub_headings(merged_chunks, options.heading_level);
        }

        // 6. Linearize tables
        merged_chunks = self.linearize_table_chunks(merged_chunks, options);

        // 7. Clean each chunk
        let mut processed: Vec<String> = merged_chunks
            .iter()
            .map(|chunk| self.clean_chunk(chunk, options))
            .filter(|c| !c.is_empty())
            .collect();

        // 8. Split oversized chunks
        if options.max_tokens > 0 {
            processed = processed
                .into_iter()
                .flat_map(|chunk| {
                    if token::count_tokens(&chunk) > options.max_tokens {
                        token::split_oversized_paragraph(&chunk, options.max_tokens)
                    } else {
                        vec![chunk]
                    }
                })
                .collect();
        }

        // 9. Prepend heading
        if let Some(heading) = section_heading {
            processed = self.prepend_heading_to_chunks(processed, heading, options);
        }

        processed
    }

    fn glue_subheadings_to_content(&self, paragraphs: Vec<String>) -> Vec<String> {
        let mut result = Vec::new();
        let re_sub = regex::Regex::new(r"^#{1,6}\s").unwrap();
        let mut i = 0;
        while i < paragraphs.len() {
            let para = &paragraphs[i];
            if i < paragraphs.len() - 1 && re_sub.is_match(para.trim_start()) {
                result.push(format!("{}\n\n{}", para, paragraphs[i + 1]));
                i += 2;
            } else {
                result.push(para.clone());
                i += 1;
            }
        }
        result
    }

    fn assemble_raw_chunks(&self, paragraphs: Vec<String>, options: &ChunkOptions) -> Vec<String> {
        let mut raw_chunks = Vec::new();
        let mut current_batch = Vec::new();
        let mut current_tokens = 0;

        for para in paragraphs {
            let pt = token::count_tokens(&para);

            if pt > options.max_tokens {
                if !current_batch.is_empty() {
                    raw_chunks.push(current_batch.join("\n\n"));
                    current_batch.clear();
                    current_tokens = 0;
                }
                raw_chunks.push(para);
                continue;
            }

            if current_tokens + pt > options.max_tokens {
                raw_chunks.push(current_batch.join("\n\n"));
                current_batch = self.compute_overlap(&current_batch, options);
                current_tokens = current_batch.iter().map(|p| token::count_tokens(p)).sum();
            }

            current_batch.push(para);
            current_tokens += pt;
        }

        if !current_batch.is_empty() {
            raw_chunks.push(current_batch.join("\n\n"));
        }

        raw_chunks
    }

    fn compute_overlap(&self, batch: &[String], options: &ChunkOptions) -> Vec<String> {
        if options.overlap_tokens == 0 || batch.is_empty() {
            return Vec::new();
        }

        let mut overlap = Vec::new();
        let mut overlap_tokens = 0;
        for i in (0..batch.len()).rev() {
            let t = token::count_tokens(&batch[i]);
            if overlap_tokens + t <= options.overlap_tokens {
                overlap.insert(0, batch[i].clone());
                overlap_tokens += t;
            } else {
                break;
            }
        }
        overlap
    }

    fn restore_tables(&self, raw_chunks: Vec<String>, tables: &[types::TableInfo], options: &ChunkOptions) -> Vec<String> {
        let mut used = std::collections::HashSet::new();
        let mut result = Vec::new();

        for chunk in raw_chunks {
            let chunk_tables: Vec<&types::TableInfo> = tables
                .iter()
                .filter(|t| chunk.contains(&t.placeholder))
                .collect();

            for t in &chunk_tables {
                used.insert(t.placeholder.clone());
            }

            if options.separate_tables_as_chunks && !chunk_tables.is_empty() {
                result.extend(self.split_chunk_around_tables(&chunk, &chunk_tables));
            } else {
                let mut restored = chunk.clone();
                for t in chunk_tables {
                    restored = restored.replace(&t.placeholder, &t.content);
                }
                result.push(restored);
            }
        }

        for t in tables {
            if !used.contains(&t.placeholder) {
                result.push(t.content.clone());
            }
        }

        let re_heading = regex::Regex::new(r"^\s*#{1,6}\s").unwrap();
        result
            .into_iter()
            .filter(|chunk| {
                let lines: Vec<&str> = chunk.lines().filter(|l| !l.trim().is_empty()).collect();
                !lines.iter().all(|line| re_heading.is_match(line))
            })
            .collect()
    }

    fn split_chunk_around_tables(&self, chunk: &str, chunk_tables: &[&types::TableInfo]) -> Vec<String> {
        let mut parts = Vec::new();
        let mut remaining = chunk;

        for t in chunk_tables {
            if let Some(idx) = remaining.find(&t.placeholder) {
                let before = remaining[..idx].trim();
                remaining = &remaining[idx + t.placeholder.len()..];

                if !before.is_empty() {
                    parts.push(before.to_string());
                }
                parts.push(t.content.clone());
            }
        }

        let after = remaining.trim();
        if !after.is_empty() {
            parts.push(after.to_string());
        }

        parts
    }

    fn split_chunks_by_sub_headings(&self, chunks: Vec<String>, heading_level: usize) -> Vec<String> {
        let sub_level = heading_level + 1;
        if sub_level > 6 {
            return chunks;
        }
        let pattern = format!(r"^#{{{},6}}\s", sub_level);
        let sub_regex = regex::Regex::new(&pattern).unwrap();
        let any_heading_regex = regex::Regex::new(r"(?m)^#{1,6}\s").unwrap();

        let mut result = Vec::new();
        for chunk in chunks {
            if !sub_regex.is_match(&chunk) && !any_heading_regex.is_match(&chunk) {
                result.push(chunk);
                continue;
            }

            let lines: Vec<&str> = chunk.split('\n').collect();
            let mut sections = Vec::new();
            let mut current = Vec::new();

            for line in lines {
                if sub_regex.is_match(line) {
                    if !current.is_empty() {
                        sections.push(current.join("\n"));
                    }
                    current = vec![line];
                } else {
                    current.push(line);
                }
            }
            if !current.is_empty() {
                sections.push(current.join("\n"));
            }

            if sections.is_empty() {
                result.push(chunk);
            } else {
                result.extend(sections);
            }
        }
        result
    }

    fn partition_table_lines(&self, chunk: &str) -> (Vec<String>, Vec<String>) {
        if !chunk.contains('|') {
            return (chunk.lines().map(|s| s.to_string()).collect(), Vec::new());
        }

        let lines = chunk.lines();
        let mut non_table_lines = Vec::new();
        let mut table_block = Vec::new();
        let mut inside_table = false;

        for line in lines {
            if line.trim().starts_with('|') {
                table_block.push(line.to_string());
                inside_table = true;
            } else if inside_table {
                inside_table = false;
                non_table_lines.push(line.to_string());
            } else {
                non_table_lines.push(line.to_string());
            }
        }
        (non_table_lines, table_block)
    }

    fn linearize_table_chunks(&self, chunks: Vec<String>, options: &ChunkOptions) -> Vec<String> {
        if !options.linearize_tables {
            return chunks;
        }

        let mut result = Vec::new();
        for chunk in chunks {
            let (non_table_lines, table_block) = self.partition_table_lines(&chunk);
            if table_block.is_empty() {
                result.push(chunk);
                continue;
            }

            let table_text = table_block.join("\n");
            let mut rows = table::linearize_table(&table_text);

            let clean_opts = ChunkOptions {
                remove_rules: options.remove_rules,
                remove_numbering: options.remove_numbering,
                remove_list_markers: options.remove_list_markers,
                remove_links: options.remove_links,
                format_latex: options.format_latex,
                remove_formatting: options.remove_formatting,
                lower_case: false,
                compact_lines: false,
                strip_heading_markers: false,
                ..Default::default()
            };
            rows = rows.iter().map(|row| self.clean_chunk(row, &clean_opts)).collect();

            let mut sub_chunks = if options.each_table_row_as_separate_chunk {
                rows
            } else {
                self.assemble_raw_chunks(rows, options)
            };

            if !non_table_lines.is_empty() && !sub_chunks.is_empty() {
                sub_chunks[0] = format!("{}\n{}", non_table_lines.join("\n"), sub_chunks[0]);
            } else if !non_table_lines.is_empty() {
                sub_chunks.push(non_table_lines.join("\n"));
            }

            if sub_chunks.is_empty() {
                result.push(chunk);
            } else {
                result.extend(sub_chunks);
            }
        }
        result
    }

    fn clean_chunk(&self, chunk: &str, options: &ChunkOptions) -> String {
        let mut c = chunk.to_string();
        if options.remove_rules {
            c = formatting::remove_horizontal_rules(&c);
        }
        if options.remove_numbering {
            c = formatting::remove_list_numbering(&c);
        }
        if options.remove_list_markers {
            c = formatting::remove_list_markers(&c);
        }
        if options.remove_links {
            c = formatting::remove_markdown_links(&c);
        }
        if options.format_latex {
            c = latex::format_latex(&c);
        }
        if options.remove_formatting {
            c = formatting::remove_markdown_formatting(&c);
        }
        if options.strip_heading_markers {
            c = formatting::strip_heading_markers(&c);
        }
        if options.lower_case {
            c = c.to_lowercase();
        }
        if options.compact_lines {
            c = formatting::remove_empty_lines(&c);
        }
        c.trim().to_string()
    }

    fn prepend_heading_to_chunks(
        &self,
        chunks: Vec<String>,
        section_heading: &str,
        options: &ChunkOptions,
    ) -> Vec<String> {
        let clean_heading = self.clean_chunk(section_heading, options).trim().to_string();
        if clean_heading.is_empty() {
            return chunks;
        }

        chunks
            .into_iter()
            .filter(|chunk| chunk != &clean_heading)
            .map(|chunk| {
                let first_line = chunk.trim_start().lines().next().unwrap_or("").trim();
                if first_line == clean_heading {
                    chunk
                } else {
                    format!("{}\n{}", clean_heading, chunk)
                }
            })
            .collect()
    }
}


