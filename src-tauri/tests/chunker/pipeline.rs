use scribo_lib::fragmenter::{
    fragment_for_embedding, fragment_for_generation, fragment_paired,
    FragmentConfig, TableInfo, CleanFlags,
};
use crate::types::validate_config_invariants;

#[test]
fn test_restore_tables() {
    let raw_chunks = vec![scribo_lib::fragmenter::pack::RawFragment {
        text: "Some text with {{TABLE_0}} and text.".to_string(),
        meta: Default::default(),
    }];
    let tables = vec![TableInfo {
        placeholder: "{{TABLE_0}}".to_string(),
        content: "| H1 |\n|---|\n| V1 |".to_string(),
        tokens: 5,
    }];
    let mut flags = CleanFlags::default();
    flags.separate_tables_as_fragments = false;
    flags.preserve_tables = true;
    
    let restored = scribo_lib::fragmenter::clean::tables::restore_tables(raw_chunks, &tables, &flags);
    assert_eq!(restored.len(), 1);
    assert!(restored[0].text.contains("| H1 |"));
    assert!(!restored[0].text.contains("{{TABLE_0}}"));
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
    
    let opts = FragmentConfig::embedding();
    let chunks = fragment_for_embedding(text, &opts);
    
    assert!(!chunks.is_empty());
    
    let flags = opts.cleaner.to_flags();
    validate_config_invariants(&chunks, &flags);
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
    
    let opts = FragmentConfig::generation();
    let chunks = fragment_for_generation(text, &opts);
    
    assert!(!chunks.is_empty());
    
    let flags = opts.cleaner.to_flags();
    validate_config_invariants(&chunks, &flags);
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
    
    let opts = FragmentConfig::structural();
    let result = fragment_paired(text.to_string(), &opts);
    
    let struct_chunks: Vec<String> = result.pairs.iter().map(|p| p.embedding.clone()).collect();
    
    assert!(!struct_chunks.is_empty());
    
    let flags = opts.cleaner.to_flags();
    validate_config_invariants(&struct_chunks, &flags);
}
