use crate::chunker::types::ChunkOptions;
use crate::chunker::markdown::formatting;
use crate::chunker::markdown::latex;

pub fn clean_chunk(chunk: &str, options: &ChunkOptions) -> String {
    type Transform = fn(&str) -> std::borrow::Cow<'_, str>;
    
    let transforms: &[(bool, Transform)] = &[
        (options.remove_rules, formatting::remove_horizontal_rules),
        (options.remove_numbering, formatting::remove_list_numbering),
        (options.remove_list_markers, formatting::remove_list_markers),
        (options.remove_links, formatting::remove_links),
        (options.format_latex, latex::format_latex),
        (options.remove_formatting, formatting::remove_markdown_formatting),
        (options.strip_heading_markers, formatting::strip_heading_markers),
        (options.compact_lines, formatting::remove_empty_lines),
    ];

    let mut c = std::borrow::Cow::Borrowed(chunk);
    
    for (enabled, transform) in transforms {
        if *enabled {
            if let std::borrow::Cow::Owned(s) = transform(&c) {
                c = std::borrow::Cow::Owned(s);
            }
        }
    }
    
    if options.lower_case {
        c = std::borrow::Cow::Owned(c.to_lowercase());
    }
    
    c.trim().to_string()
}
