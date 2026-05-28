use scribo_lib::fragmenter::segment::heading::split_by_headings;

// Helper to wrap extract_yaml_frontmatter check
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
    
    let sections = split_by_headings(text, 2);
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0].0, "# H1\nContent 1");
    assert_eq!(sections[1].0, "## H2\nContent 2");
    assert_eq!(sections[2].0, "## H2-2\nContent 3");
}

#[test]
fn test_split_by_headings_any_level() {
    let text = "# H1\nContent 1\n## H2\nContent 2\n### H3\nContent 3";
    
    let sections = split_by_headings(text, 0);
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0].0, "# H1\nContent 1");
    assert_eq!(sections[1].0, "## H2\nContent 2");
    assert_eq!(sections[2].0, "### H3\nContent 3");
}

#[test]
fn test_split_by_headings_none() {
    let text = "This is a normal paragraph with no headings.";
    let sections = split_by_headings(text, 2);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].0, text);
}

#[test]
fn test_split_by_headings_carriage_return() {
    let text = "# H1\r\nContent 1\r\n## H2\r\nContent 2";
    let sections = split_by_headings(text, 2);
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].0, "# H1\r\nContent 1");
    assert_eq!(sections[1].0, "## H2\r\nContent 2");
}
