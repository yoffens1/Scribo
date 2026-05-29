use crate::fragmenter::config::{CleanFlags, LatexHandling};
use super::{markdown, latex};

pub fn apply(fragment: &str, flags: &CleanFlags) -> String {
    use std::borrow::Cow;
    type Transform = fn(&str) -> Cow<'_, str>;

    let mut text_str = fragment.to_string();
    if flags.linearize_tables {
        let (body, tables) = crate::fragmenter::clean::tables::extract_tables(&text_str);
        if !tables.is_empty() {
            let mut restored = body;
            for t in tables {
                let linearized_rows = crate::fragmenter::clean::tables::linearize_table(&t.content);
                let replacement = linearized_rows.join("\n");
                restored = restored.replace(&t.placeholder, &replacement);
            }
            text_str = restored;
        }
    }

    let mut transforms: Vec<(bool, Transform)> = vec![
        (flags.remove_rules,           markdown::remove_horizontal_rules as Transform),
        (flags.remove_numbering,       markdown::remove_list_numbering as Transform),
        (flags.remove_list_markers,    markdown::remove_list_markers as Transform),
        (flags.remove_links,           markdown::remove_links as Transform),
    ];

    match flags.latex {
        LatexHandling::Format => {
            transforms.push((true, latex::format_latex as Transform));
        }
        LatexHandling::Remove => {
            transforms.push((true, latex::remove_latex as Transform));
        }
        LatexHandling::Keep => {}
    }

    transforms.push((flags.remove_formatting,      markdown::remove_markdown_formatting as Transform));
    transforms.push((flags.strip_heading_markers,  markdown::strip_heading_markers as Transform));
    transforms.push((flags.compact_lines,          markdown::collapse_blank_lines as Transform));

    let mut text = Cow::Owned(text_str);
    for (enabled, f) in transforms {
        if enabled {
            if let Cow::Owned(s) = f(&text) {
                text = Cow::Owned(s);
            }
        }
    }

    if flags.lower_case {
        text = Cow::Owned(text.to_lowercase());
    }

    text.trim().to_string()
}
