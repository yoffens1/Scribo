use scribo_lib::chunker::extract::{extract_yaml_frontmatter, split_by_headings};

#[test]
fn test_extract_yaml_frontmatter_valid() {
    let content = "---\ntitle: \"My Note\"\ntags: [a, b]\n---\nHello World";
    let (metadata, remaining) = extract_yaml_frontmatter(content);
    
    assert!(metadata.is_some());
    let map = metadata.unwrap();
    assert_eq!(map.get("title").and_then(|v| v.as_str()), Some("My Note"));
    assert_eq!(remaining, "Hello World");
}

#[test]
fn test_extract_yaml_frontmatter_invalid() {
    let content = "---\nthis is not yaml : { : [\n---\nHello World";
    let (metadata, remaining) = extract_yaml_frontmatter(content);
    
    assert!(metadata.is_none());
    assert_eq!(remaining, "Hello World");
}

#[test]
fn test_extract_yaml_frontmatter_missing() {
    let content = "Hello World\n---\nNo frontmatter here";
    let (metadata, remaining) = extract_yaml_frontmatter(content);
    
    assert!(metadata.is_none());
    assert_eq!(remaining, content);
}

#[test]
fn test_split_by_headings_specific_level() {
    let text = "# H1\nContent 1\n## H2\nContent 2\n## H2-2\nContent 3";
    
    // Split by level 2 (##)
    let sections = split_by_headings(text, 2);
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0], "# H1\nContent 1");
    assert_eq!(sections[1], "## H2\nContent 2");
    assert_eq!(sections[2], "## H2-2\nContent 3");
}

#[test]
fn test_split_by_headings_any_level() {
    let text = "# H1\nContent 1\n## H2\nContent 2\n### H3\nContent 3";
    
    // Split by level 0 (meaning any heading level)
    let sections = split_by_headings(text, 0);
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0], "# H1\nContent 1");
    assert_eq!(sections[1], "## H2\nContent 2");
    assert_eq!(sections[2], "### H3\nContent 3");
}

#[test]
fn test_split_by_headings_none() {
    let text = "This is a normal paragraph with no headings.";
    let sections = split_by_headings(text, 2);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0], text);
}

#[test]
fn test_split_by_headings_carriage_return() {
    let text = "# H1\r\nContent 1\r\n## H2\r\nContent 2";
    let sections = split_by_headings(text, 2);
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0], "# H1\r\nContent 1");
    assert_eq!(sections[1], "## H2\r\nContent 2");
}
