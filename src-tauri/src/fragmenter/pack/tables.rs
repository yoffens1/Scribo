use crate::fragmenter::config::CleanFlags;
use crate::fragmenter::clean::tables::{
    TableInfo, RE_TABLE_PLACEHOLDER, RE_HEADING_RESTORE, linearize_table,
};
use super::RawFragment;

pub fn restore_tables(
    raw_fragments: Vec<RawFragment>,
    tables: &[TableInfo],
    flags: &CleanFlags,
) -> Vec<RawFragment> {
    let mut used = std::collections::HashSet::new();
    let mut result = Vec::new();

    for fragment in raw_fragments {
        let mut fragment_tables = Vec::new();
        for cap in RE_TABLE_PLACEHOLDER.captures_iter(&fragment.text) {
            let placeholder = cap[0].to_string();
            if let Some(t) = tables.iter().find(|t| t.placeholder == placeholder) {
                if !fragment_tables.iter().any(|existing: &&TableInfo| existing.placeholder == placeholder) {
                    fragment_tables.push(t);
                }
            }
        }

        for t in &fragment_tables {
            used.insert(t.placeholder.clone());
        }

        if flags.separate_tables_as_fragments && !fragment_tables.is_empty() {
            let mut current_texts = vec![fragment.text.clone()];
            for t in &fragment_tables {
                let mut next_texts = Vec::new();
                for txt in current_texts {
                    if let Some(idx) = txt.find(&t.placeholder) {
                        let before = txt[..idx].trim().to_string();
                        let after = txt[idx + t.placeholder.len()..].trim().to_string();
                        if !before.is_empty() {
                            next_texts.push(before);
                        }
                        next_texts.push(t.placeholder.clone());
                        if !after.is_empty() {
                            next_texts.push(after);
                        }
                    } else {
                        next_texts.push(txt);
                    }
                }
                current_texts = next_texts;
            }

            for txt in current_texts {
                if txt.starts_with("{{TABLE_") && txt.ends_with("}}") {
                    if let Some(t) = tables.iter().find(|t| t.placeholder == txt) {
                        if flags.linearize_tables && flags.each_table_row_as_separate_fragment {
                            let linearized_rows = linearize_table(&t.content);
                            for row in linearized_rows {
                                result.push(RawFragment {
                                    text: row,
                                    meta: fragment.meta.clone(),
                                });
                            }
                        } else {
                            result.push(RawFragment {
                                text: t.content.clone(),
                                meta: fragment.meta.clone(),
                            });
                        }
                    }
                } else {
                    result.push(RawFragment {
                        text: txt,
                        meta: fragment.meta.clone(),
                    });
                }
            }
        } else {
            let mut restored = fragment.text.clone();
            for t in fragment_tables {
                restored = restored.replace(&t.placeholder, &t.content);
            }
            result.push(RawFragment {
                text: restored,
                meta: fragment.meta.clone(),
            });
        }
    }

    for t in tables {
        if !used.contains(&t.placeholder) {
            if flags.linearize_tables && flags.each_table_row_as_separate_fragment {
                let linearized_rows = linearize_table(&t.content);
                for row in linearized_rows {
                    result.push(RawFragment {
                        text: row,
                        meta: Default::default(),
                    });
                }
            } else {
                result.push(RawFragment {
                    text: t.content.clone(),
                    meta: Default::default(),
                });
            }
        }
    }

    result
        .into_iter()
        .filter(|fragment| {
            let lines: Vec<&str> = fragment.text.lines().filter(|l| !l.trim().is_empty()).collect();
            !lines.iter().all(|line| RE_HEADING_RESTORE.is_match(line))
        })
        .collect()
}
