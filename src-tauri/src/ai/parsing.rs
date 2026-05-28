//! # JSON Parsing Utilities
//!
//! Post-processes raw LLM text responses to extract structured JSON.
//!
//! LLMs frequently wrap their JSON output in markdown code fences (` ```json ... ``` `)
//! or add explanatory prose around the JSON. These utilities strip the noise and return
//! only the outermost JSON structure.
//!
//! ## Strategy
//!
//! All three functions use the same bracket-scan heuristic:
//! find the **first** opening bracket/brace and the **last** closing bracket/brace,
//! then return the substring between them (inclusive).
//! This handles:
//! - Clean JSON with no surrounding text.
//! - JSON wrapped in ` ```json ``` ` fences.
//! - Explanatory text before/after the JSON.
//!
//! It does **not** validate that the extracted string is valid JSON —
//! callers are responsible for `serde_json::from_str`.

/// Extracts the outermost JSON value (object or array) from `raw`.
///
/// Returns `None` if no `{`/`[` ... `}`/`]` pair is found.
pub fn extract_json_payload(raw: &str) -> Option<String> {
    let start_idx = raw.find(|c| c == '{' || c == '[')?;
    let end_idx = raw.rfind(|c| c == '}' || c == ']')?;
    if end_idx > start_idx {
        Some(raw[start_idx..=end_idx].to_string())
    } else {
        None
    }
}

/// Extracts the outermost JSON **object** (`{ ... }`) from `raw`.
/// Falls back to returning all of `raw` if no matching braces are found.
pub fn extract_json_object(raw: &str) -> &str {
    extract_between(raw, '{', '}')
}

/// Extracts the outermost JSON **array** (`[ ... ]`) from `raw`.
/// Falls back to returning all of `raw` if no matching brackets are found.
pub fn extract_json_array(raw: &str) -> &str {
    extract_between(raw, '[', ']')
}

/// Returns the substring of `raw` from the first `open` to the last `close` (inclusive).
/// If no valid pair is found, returns `raw` unchanged.
fn extract_between(raw: &str, open: char, close: char) -> &str {
    if let (Some(s), Some(e)) = (raw.find(open), raw.rfind(close)) {
        if e >= s {
            return &raw[s..=e];
        }
    }
    raw
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

    #[test]
    fn test_extract_json_object_helper() {
        let raw = "some text before {\"a\": 1} some text after";
        assert_eq!(extract_json_object(raw), "{\"a\": 1}");
    }
}
