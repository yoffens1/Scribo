use scribo_lib::chunker::*;

#[test]
fn test_chunker_basic() {
    let options = ChunkOptions {
        max_tokens: 50,
        ..Default::default()
    };
    let chunker = Chunker::new(options);

    let content = "
# Title
This is a short paragraph.

This is another paragraph that should be kept separate if they are long, but since they are short, they might be batched together depending on the token count.

## Subheading
Some more text here.
    ".trim().to_string();

    let result = chunker.chunk_paired(content);
    assert!(!result.pairs.is_empty(), "Should generate at least one chunk");
    
    // Check that we have valid embedding and generation text
    for pair in result.pairs {
        assert!(!pair.generation.is_empty());
        assert!(!pair.embedding.is_empty());
    }
}

#[test]
fn test_chunker_with_table() {
    let options = ChunkOptions {
        max_tokens: 100,
        preserve_tables: true,
        linearize_tables: true,
        ..Default::default()
    };
    let chunker = Chunker::new(options);

    let content = "
# Data
Here is a table:

| Name | Age |
|---|---|
| Alice | 30 |
| Bob | 25 |
    ".trim().to_string();

    let result = chunker.chunk_paired(content);
    
    // Print all chunks to debug if this fails
    for pair in &result.pairs {
        println!("CHUNK:\n{}\n", pair.generation);
    }
    
    let has_linearized = result.pairs.iter().any(|p| p.generation.contains("alice") && p.generation.contains("30"));
    assert!(has_linearized, "Table should be linearized into sentences");
}

#[test]
fn test_chunker_latex() {
    let options = ChunkOptions {
        format_latex: true,
        ..Default::default()
    };
    let chunker = Chunker::new(options);

    let content = "Math: $\\alpha + \\beta = \\sum_{i=0}^{\\infty} x_i$".to_string();
    
    // Получаем чанки только для embedding
    let chunks = chunker.chunk_for_embedding(&content);
    let embedding = &chunks[0];
    
    assert!(embedding.contains("α + β"), "LaTeX greek letters should be formatted");
    assert!(embedding.contains("∑"), "LaTeX symbols should be formatted");
}

#[test]
fn test_explicit_embedding_config() {
    // Явно используем конфиг merge_with_embedding для инициализации чанкера
    let config = ChunkOptions::default().merge_with_embedding();
    let chunker = Chunker::new(config);

    let content = "# My Heading\nHere is a [[WikiLink]] and some **bold text**!".to_string();
    
    // Получаем чанки только для embedding (без пары)
    let chunks = chunker.chunk_for_embedding(&content);
    
    assert_eq!(chunks.len(), 1, "Should produce exactly 1 chunk");
    assert_eq!(
        chunks[0], "my heading\nhere is a wikilink and some bold text!",
        "The text should be fully lowercased, stripped of headings, markdown, and links according to merge_with_embedding"
    );
}
