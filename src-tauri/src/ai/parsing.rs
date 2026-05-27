pub fn extract_json_payload(raw: &str) -> Option<String> {
    let start_idx = raw.find('{')?;
    let end_idx = raw.rfind('}')?;
    if end_idx > start_idx {
        Some(raw[start_idx..=end_idx].to_string())
    } else {
        None
    }
}
