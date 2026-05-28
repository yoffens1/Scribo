use scribo_lib::fragmenter::clean::tables::{extract_tables, linearize_table};

#[test]
fn test_extract_tables_valid() {
    let text = "Intro\n\n| Col1 | Col2 |\n|---|---|\n| val1 | val2 |\n\nOutro";
    let (cleaned, tables) = extract_tables(text);
    
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].placeholder, "{{TABLE_0}}");
    assert!(tables[0].content.contains("| Col1 | Col2 |"));
    assert!(cleaned.contains("{{TABLE_0}}"));
    assert!(!cleaned.contains("| Col1 | Col2 |"));
}

#[test]
fn test_extract_tables_ignore_regular_pipes() {
    let text = "This is not a | table | line because there is no separator row.";
    let (cleaned, tables) = extract_tables(text);
    
    assert_eq!(tables.len(), 0);
    assert_eq!(cleaned, text);
}

#[test]
fn test_linearize_table_valid() {
    let table = "| Header1 | Header2 |\n|---|---|\n| Row1Col1 | Row1Col2 |\n| Row2Col1 | Row2Col2 |";
    let result = linearize_table(table);
    
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], "1. Header1: Row1Col1. Header2: Row1Col2");
    assert_eq!(result[1], "2. Header1: Row2Col1. Header2: Row2Col2");
}

#[test]
fn test_linearize_table_empty_cells() {
    let table = "| Header1 | Header2 |\n|---|---|\n| | Row1Col2 |";
    let result = linearize_table(table);
    
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "1. Header2: Row1Col2");
}
