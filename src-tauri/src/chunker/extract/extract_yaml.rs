use std::collections::HashMap;

pub fn extract_yaml_frontmatter(content: &str) -> (Option<HashMap<String, serde_json::Value>>, String) {
    if let Some(stripped) = content.strip_prefix("---\n") {
        if let Some(end_idx) = stripped.find("\n---\n") {
            let yaml_text = &stripped[..end_idx];
            let remaining = stripped[end_idx + 5..].to_string();
            let metadata = simple_yaml_parse(yaml_text);
            return (Some(metadata), remaining);
        }
    }
    (None, content.to_string())
}

fn simple_yaml_parse(yaml_text: &str) -> HashMap<String, serde_json::Value> {
    let mut result = HashMap::new();
    let mut current_key: Option<String> = None;

    for line in yaml_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.contains(':') && !trimmed.starts_with('-') {
            if let Some(colon_idx) = trimmed.find(':') {
                let key = trimmed[..colon_idx].trim().to_string();
                let val_str = trimmed[colon_idx + 1..].trim().to_string();
                
                let value = if val_str == "true" {
                    serde_json::Value::Bool(true)
                } else if val_str == "false" {
                    serde_json::Value::Bool(false)
                } else if let Ok(num) = val_str.parse::<f64>() {
                    if let Some(n) = serde_json::Number::from_f64(num) {
                        serde_json::Value::Number(n)
                    } else {
                        serde_json::Value::String(val_str)
                    }
                } else if val_str.is_empty() {
                    continue;
                } else {
                    serde_json::Value::String(val_str)
                };
                
                result.insert(key.clone(), value);
                current_key = Some(key);
            }
        } else if let Some(stripped) = trimmed.strip_prefix('-') {
            if let Some(ref key) = current_key {
                let list_item = stripped.trim().to_string();
                let item_val = if list_item == "true" {
                    serde_json::Value::Bool(true)
                } else if list_item == "false" {
                    serde_json::Value::Bool(false)
                } else if let Ok(num) = list_item.parse::<f64>() {
                    if let Some(n) = serde_json::Number::from_f64(num) {
                        serde_json::Value::Number(n)
                    } else {
                        serde_json::Value::String(list_item)
                    }
                } else {
                    serde_json::Value::String(list_item)
                };

                let entry = result.entry(key.clone()).or_insert(serde_json::Value::Array(Vec::new()));
                if let serde_json::Value::Array(ref mut arr) = entry {
                    arr.push(item_val);
                }
            }
        }
    }

    result
}
