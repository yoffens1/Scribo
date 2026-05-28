use crate::fragmenter::config::CleanFlags;
use crate::fragmenter::clean::tables::{
    TableInfo, RE_TABLE_PLACEHOLDER, RE_HEADING_RESTORE, split_fragment_around_tables,
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
            let split_texts = split_fragment_around_tables(&fragment.text, &fragment_tables);
            for t in split_texts {
                result.push(RawFragment {
                    text: t,
                    meta: fragment.meta.clone(),
                });
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
            result.push(RawFragment {
                text: t.content.clone(),
                meta: Default::default(),
            });
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

pub fn linearize_table_fragments(
    fragments: Vec<RawFragment>,
    flags: &CleanFlags,
    max_tokens: usize,
) -> Vec<RawFragment> {
    if !flags.linearize_tables {
        return fragments;
    }

    let mut result = Vec::new();
    for fragment in fragments {
        let (before, table_block, after) = crate::fragmenter::clean::tables::partition_table_lines(&fragment.text);
        if table_block.is_empty() {
            result.push(fragment);
            continue;
        }

        let table_text = table_block.join("\n");
        let mut rows = crate::fragmenter::clean::tables::linearize_table(&table_text);

        let mut row_clean_flags = flags.clone();
        row_clean_flags.lower_case = false;
        row_clean_flags.compact_lines = false;
        row_clean_flags.strip_heading_markers = false;
        rows = rows.iter().map(|row| crate::fragmenter::clean::apply(row, &row_clean_flags)).collect();

        let sub_fragments_text = if flags.each_table_row_as_separate_fragment {
            rows
        } else {
            // We can pack them back using a token batch aggregator
            let sub_atoms = rows.iter().map(|r| crate::fragmenter::segment::Atom::whole(r)).collect();
            let packed = super::token_budget::pack(sub_atoms, max_tokens, 0);
            packed.into_iter().map(|rf| rf.text).collect()
        };

        let mut sub_fragments: Vec<RawFragment> = sub_fragments_text
            .into_iter()
            .map(|t| RawFragment {
                text: t,
                meta: fragment.meta.clone(),
            })
            .collect();

        if !before.is_empty() && !sub_fragments.is_empty() {
            sub_fragments[0].text = format!("{}\n{}", before.join("\n"), sub_fragments[0].text);
        } else if !before.is_empty() {
            sub_fragments.push(RawFragment {
                text: before.join("\n"),
                meta: fragment.meta.clone(),
            });
        }

        if !after.is_empty() && !sub_fragments.is_empty() {
            let last_idx = sub_fragments.len() - 1;
            sub_fragments[last_idx].text = format!("{}\n{}", sub_fragments[last_idx].text, after.join("\n"));
        } else if !after.is_empty() {
            sub_fragments.push(RawFragment {
                text: after.join("\n"),
                meta: fragment.meta.clone(),
            });
        }

        if sub_fragments.is_empty() {
            result.push(fragment);
        } else {
            result.extend(sub_fragments);
        }
    }
    result
}
