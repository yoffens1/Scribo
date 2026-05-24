use scribo_lib::chunker::pipeline::assemble::{glue_subheadings_to_content, assemble_raw_chunks};
use scribo_lib::chunker::pipeline::tables::restore_tables;
use scribo_lib::chunker::{chunk_for_embedding, chunk_for_generation, chunk_paired, ChunkOptions, TableInfo};
use crate::types::validate_config_invariants;

#[test]
fn test_glue_subheadings_to_content() {
    let paragraphs = vec!["## Subheading", "Content under subheading", "Normal paragraph"];
    let glued = glue_subheadings_to_content(paragraphs);
    
    assert_eq!(glued.len(), 2);
    assert_eq!(glued[0], "## Subheading\n\nContent under subheading");
    assert_eq!(glued[1], "Normal paragraph");
}

#[test]
fn test_assemble_raw_chunks_no_overlap() {
    let paragraphs = vec![
        std::borrow::Cow::Borrowed("Paragraph one"),
        std::borrow::Cow::Borrowed("Paragraph two"),
    ];
    let opts = ChunkOptions {
        max_tokens: 50,
        overlap_tokens: 0,
        ..ChunkOptions::default()
    };
    
    let chunks = assemble_raw_chunks(paragraphs, &opts);
    assert!(!chunks.is_empty());
}

#[test]
fn test_restore_tables() {
    let raw_chunks = vec!["Some text with {{TABLE_0}} and text.".to_string()];
    let tables = vec![TableInfo {
        placeholder: "{{TABLE_0}}".to_string(),
        content: "| H1 |\n|---|\n| V1 |".to_string(),
        tokens: 5,
    }];
    let opts = ChunkOptions {
        separate_tables_as_chunks: false,
        ..ChunkOptions::default()
    };
    
    let restored = restore_tables(raw_chunks, &tables, &opts);
    assert_eq!(restored.len(), 1);
    assert!(restored[0].contains("| H1 |"));
    assert!(!restored[0].contains("{{TABLE_0}}"));
}

#[test]
fn test_pipeline_embedding() {
    let text = r#"---
title: Embedding Test
---
# Main Header
Intro paragraph with some text.

## Subsection
Here is some list items:
- Item 1
- Item 2

| Table Header |
|---|
| Row Value |

Math inline $ \alpha + \beta $ and block:
$$
\sum_{i=1}^n
$$
"#;
    
    let opts = ChunkOptions::default(); // default uses embedding preset logic in chunk_for_embedding
    let chunks = chunk_for_embedding(text, &opts);
    
    assert!(!chunks.is_empty());
    
    // Dynamically validate all active config invariants for the embedding mode preset
    let embedding_opts = opts.for_mode(scribo_lib::chunker::ChunkMode::Embedding);
    validate_config_invariants(&chunks, &embedding_opts);
}

#[test]
fn test_pipeline_generation() {
    let text = r#"---
title: Generation Test
---
# Main Header
Intro paragraph for generation.

## Subsection
- Item 1
- Item 2
"#;
    
    let opts = ChunkOptions::default();
    let chunks = chunk_for_generation(text, &opts);
    
    assert!(!chunks.is_empty());
    
    // Dynamically validate all active config invariants for the generation mode preset
    let gen_opts = opts.for_mode(scribo_lib::chunker::ChunkMode::Generation);
    validate_config_invariants(&chunks, &gen_opts);
}

#[test]
fn test_pipeline_structural() {
    let text = r#"---
title: Structural Test
---
# Main Header
Intro paragraph for structural.

## Subsection
- Item 1
- Item 2
"#;
    
    let opts = ChunkOptions::default();
    let result = chunk_paired(text.to_string(), &opts);
    
    let struct_chunks: Vec<String> = result.pairs.iter().map(|p| p.embedding.clone()).collect();
    
    assert!(!struct_chunks.is_empty());
    
    // Dynamically validate all active config invariants for structural mode (which matches struct_chunks raw parsing options)
    let struct_opts = opts.for_mode(scribo_lib::chunker::ChunkMode::Structural);
    validate_config_invariants(&struct_chunks, &struct_opts);
}
