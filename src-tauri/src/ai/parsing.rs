pub fn extract_json_payload(raw: &str) -> Option<String> {
    let start_idx = raw.find(|c| c == '{' || c == '[')?;
    let end_idx = raw.rfind(|c| c == '}' || c == ']')?;
    if end_idx > start_idx {
        Some(raw[start_idx..=end_idx].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_clean_json() {
        let raw = r#"{"name": "test"}"#;
        assert_eq!(extract_json_payload(raw), Some(raw.to_string()));
    }

    #[test]
    fn test_extract_json_with_code_block() {
        let raw = r#"Here is the result:
```json
{"name": "test", "items": [1, 2]}
```
Hope this helps!"#;
        assert_eq!(extract_json_payload(raw), Some(r#"{"name": "test", "items": [1, 2]}"#.to_string()));
    }

    #[test]
    fn test_extract_invalid_json_no_brackets() {
        let raw = "no brackets here";
        assert_eq!(extract_json_payload(raw), None);
    }

    #[test]
    fn test_extract_single_bracket() {
        let raw = "{";
        assert_eq!(extract_json_payload(raw), None);
    }

    #[test]
    fn test_extract_json_array() {
        let raw = r#"```json
[{"name": "test"}]
```"#;
        assert_eq!(extract_json_payload(raw), Some(r#"[{"name": "test"}]"#.to_string()));
    }
}

