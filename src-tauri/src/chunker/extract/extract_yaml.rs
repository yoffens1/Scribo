pub fn extract_yaml_frontmatter(content: &str) -> (Option<serde_json::Map<String, serde_json::Value>>, String) {
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
